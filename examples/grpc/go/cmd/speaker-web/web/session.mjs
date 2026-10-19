// State mutations are serialized. A color gesture owns at most one queued value.
// displayQuote is explicitly the last confirmed quote while a new one is pending.
export function createSession({ send, changed, failed, save }) {
    const session = {
        result: null,
        focus: null,
        quote: null,
        displayQuote: null,
        quoteStatus: "idle",
        pending: 0,
    };
    let sequence = 0,
        controller = null,
        running = false,
        quoteTimer = null;
    const queue = [],
        idle = [];
    const identity = () =>
        JSON.stringify([
            session.result?.state.modelDigest,
            session.result?.state.revision,
            session.focus,
        ]);
    function invalidate() {
        sequence++;
        controller?.abort();
        clearTimeout(quoteTimer);
        quoteTimer = null;
        session.quote = null;
        session.quoteStatus = "loading";
    }
    function scheduleQuote(delay = 0) {
        clearTimeout(quoteTimer);
        quoteTimer = null;
        if (delay) quoteTimer = setTimeout(() => void refreshQuote(), delay);
        else void refreshQuote();
    }
    async function refreshQuote() {
        if (!session.result || running || queue.length) return;
        invalidate();
        const seq = sequence,
            key = identity(),
            state = session.result.state;
        controller = new AbortController();
        const signal = controller.signal;
        const request = { schemaVersion: "1.0", state };
        if (session.focus) request.focusQuestionId = session.focus;
        changed(session);
        try {
            const payload = await send("quote", { quote: request }, signal);
            if (seq !== sequence || key !== identity()) return;
            const q = payload.quote;
            if (
                !q ||
                q.modelDigest !== state.modelDigest ||
                q.revision !== state.revision ||
                (session.focus && q.focus?.question.id !== session.focus)
            )
                throw new Error("报价身份不匹配");
            session.quote = q;
            session.displayQuote = q;
            session.quoteStatus = "settled";
            changed(session);
        } catch (e) {
            if (seq === sequence && e.name !== "AbortError") {
                session.quoteStatus = "failed";
                changed(session);
                failed(e);
            }
        }
    }
    function accept(result, keepFocus) {
        session.result = result;
        const path = result.snapshot.flow.path;
        if (!keepFocus || !path.includes(session.focus))
            session.focus = result.snapshot.flow.nextQuestionId;
        save(result.state);
        changed(session); // Confirmed Visual never waits for Quote.
    }
    async function drain() {
        if (running) return;
        running = true;
        let quietDelay = 0;
        while (queue.length) {
            const item = queue.shift();
            session.pending = queue.length + 1;
            quietDelay = item.color && !item.final ? 120 : 0;
            try {
                const state = session.result.state;
                const value =
                    typeof item.value === "function"
                        ? item.value(state)
                        : item.value;
                // input + change often report the same final color. Do not send a no-op.
                if (!item.color || state.answers[item.questionId] !== value) {
                    const event = {
                        schemaVersion: "1.0",
                        type: item.type,
                        questionId: item.questionId,
                        expectedRevision: state.revision,
                    };
                    if (item.type === "set_answer") event.value = value;
                    const payload = await send("apply", { state, event });
                    accept(payload.result, item.keepFocus);
                }
            } catch (e) {
                failed(e);
            } finally {
                item.resolve();
                session.pending = queue.length;
                changed(session);
            }
        }
        running = false;
        session.pending = 0;
        scheduleQuote(quietDelay);
        for (const resolve of idle.splice(0)) resolve();
    }
    function enqueue(questionId, value, keepFocus, type, color, final = true) {
        if (
            color &&
            !running &&
            !queue.length &&
            session.result?.state.answers[questionId] === value
        ) {
            if (final && !session.quote) scheduleQuote();
            return Promise.resolve();
        }
        invalidate();
        let deferred;
        if (color) {
            const index = queue.findIndex(
                (item) => item.color && item.questionId === questionId,
            );
            // Move the replacement to the tail so intervening non-color actions keep
            // their order relative to the user's newest color selection.
            if (index >= 0) deferred = queue.splice(index, 1)[0];
        }
        if (!deferred) {
            deferred = {};
            deferred.completion = new Promise((resolve) => {
                deferred.resolve = resolve;
            });
        }
        queue.push({
            questionId,
            value,
            keepFocus,
            type,
            color,
            final,
            resolve: deferred.resolve,
            completion: deferred.completion,
        });
        session.pending = queue.length + Number(running);
        changed(session);
        void drain();
        return deferred.completion;
    }
    session.restore = async (state) => {
        invalidate();
        const payload = await send(
            state ? "evaluate" : "initial",
            state ? { state } : {},
        );
        session.displayQuote = null;
        accept(payload.result, false);
        void refreshQuote();
    };
    session.navigate = (focus) => {
        if (
            focus !== null &&
            !session.result.snapshot.flow.path.includes(focus)
        )
            return;
        session.focus = focus;
        invalidate();
        changed(session);
        void refreshQuote();
    };
    session.select = (
        questionId,
        value,
        keepFocus = true,
        type = "set_answer",
    ) => enqueue(questionId, value, keepFocus, type, false);
    session.selectColor = (questionId, value, final = true) =>
        enqueue(questionId, value, true, "set_answer", true, final);
    session.settle = () =>
        running || queue.length
            ? new Promise((resolve) => idle.push(resolve))
            : Promise.resolve();
    return session;
}
