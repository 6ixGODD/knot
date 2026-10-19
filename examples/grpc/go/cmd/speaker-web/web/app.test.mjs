import test from "node:test";
import assert from "node:assert/strict";
import { createSession } from "./session.mjs";
const tick = () => new Promise((r) => setImmediate(r));
function result(revision = 0, answers = {}) {
    return {
        state: { modelDigest: "digest", revision, answers, skipped: [] },
        snapshot: {
            flow: { path: ["a", "b"], nextQuestionId: "b" },
            visual: { targets: [] },
        },
    };
}
test("serialized Apply uses latest revision and renders before Quote completes", async () => {
    let active = 0,
        max = 0;
    const revisions = [],
        renders = [];
    const s = createSession({
        send: async (method, body) => {
            if (method === "initial") return { result: result() };
            if (method === "quote") return new Promise(() => {});
            active++;
            max = Math.max(max, active);
            revisions.push(body.event.expectedRevision);
            await tick();
            active--;
            return {
                result: result(body.state.revision + 1, {
                    b: body.event.value,
                }),
            };
        },
        changed: (s) => renders.push(s.result?.state.revision),
        failed: (e) => {
            throw e;
        },
        save: () => {},
    });
    await s.restore(null);
    s.select("b", "one");
    s.select("b", (state) => state.answers.b + "two");
    await s.settle();
    assert.equal(max, 1);
    assert.deepEqual(revisions, [0, 1]);
    assert.equal(s.result.state.answers.b, "onetwo");
    assert.ok(renders.includes(2));
});
test("late Quote cannot overwrite revision or focus; navigation never mutates State", async () => {
    const waiting = [];
    const s = createSession({
        send: async (method, body) => {
            if (method === "initial") return { result: result() };
            if (method === "quote")
                return new Promise((resolve) =>
                    waiting.push({ resolve, request: body.quote }),
                );
            return { result: result(1, { b: "new" }) };
        },
        changed: () => {},
        failed: (e) => {
            throw e;
        },
        save: () => {},
    });
    await s.restore(null);
    const original = JSON.stringify(s.result.state);
    s.navigate("a");
    assert.equal(JSON.stringify(s.result.state), original);
    waiting[1].resolve({
        quote: {
            modelDigest: "digest",
            revision: 0,
            focus: { question: { id: "a" } },
            price: { status: "range" },
        },
    });
    await tick();
    assert.equal(s.quote.focus.question.id, "a");
    waiting[0].resolve({
        quote: {
            modelDigest: "digest",
            revision: 0,
            focus: { question: { id: "b" } },
        },
    });
    await tick();
    assert.equal(s.quote.focus.question.id, "a");
    await s.select("b", "new");
    s.navigate("b");
    const last = waiting.at(-1);
    last.resolve({
        quote: {
            modelDigest: "digest",
            revision: 1,
            focus: { question: { id: "b" } },
        },
    });
    await tick();
    waiting[2].resolve({
        quote: {
            modelDigest: "digest",
            revision: 1,
            focus: { question: { id: "a" } },
        },
    });
    await tick();
    assert.equal(s.quote.focus.question.id, "b");
});
test("Apply failure preserves successful State and recovery does not replace failed saved State", async () => {
    let initial = 0;
    const errors = [];
    const s = createSession({
        send: async (method) => {
            if (method === "initial") {
                initial++;
                return { result: result() };
            }
            if (method === "quote") return new Promise(() => {});
            throw new Error("invalid");
        },
        changed: () => {},
        failed: (e) => errors.push(e.message),
        save: () => {},
    });
    await s.restore(null);
    await s.select("b", "bad");
    assert.equal(s.result.state.revision, 0);
    assert.deepEqual(errors, ["invalid"]);
    await assert.rejects(s.restore({ revision: 99 }));
    assert.equal(initial, 1);
});
test("Quote failure settles as failed while the committed Visual is retained", async () => {
    const errors = [];
    const s = createSession({
        send: async (method, body) => {
            if (method === "quote") throw new Error("quote failed");
            return {
                result: result(
                    method === "apply" ? body.state.revision + 1 : 0,
                ),
            };
        },
        changed: () => {},
        failed: (e) => errors.push(e.message),
        save: () => {},
    });
    await s.restore(null);
    await tick();
    assert.equal(s.quoteStatus, "failed");
    await s.select("b", "new");
    await tick();
    assert.equal(s.result.state.revision, 1);
    assert.equal(s.quoteStatus, "failed");
    assert.equal(s.quote, null);
    assert.equal(errors.length, 2);
});
test("color burst retains only one in-flight and the latest pending value, then quotes once", async () => {
    const requests = [],
        waiters = [],
        quotes = [],
        frames = [];
    const s = createSession({
        send: async (method, body) => {
            if (method === "initial") return { result: result() };
            if (method === "quote") {
                quotes.push(body.quote);
                return {
                    quote: {
                        modelDigest: "digest",
                        revision: body.quote.state.revision,
                        focus: { question: { id: "b" } },
                        price: {
                            status: "range",
                            minMinor: 100,
                            maxMinor: 200,
                        },
                    },
                };
            }
            requests.push(body.event);
            return new Promise((resolve) =>
                waiters.push(() =>
                    resolve({
                        result: result(body.state.revision + 1, {
                            b: body.event.value,
                        }),
                    }),
                ),
            );
        },
        changed: (s) =>
            frames.push({ pending: s.pending, price: s.displayQuote?.price }),
        failed: (e) => {
            throw e;
        },
        save: () => {},
    });
    await s.restore(null);
    await tick();
    const confirmed = s.displayQuote;
    quotes.length = 0;
    s.selectColor("b", "#000001", false);
    let pending;
    for (let i = 2; i <= 30; i++) {
        const next = s.selectColor(
            "b",
            "#" + i.toString(16).padStart(6, "0"),
            false,
        );
        if (pending) assert.equal(next, pending);
        pending = next;
        assert.equal(s.pending, 2);
    }
    s.selectColor("b", "#00001e", true);
    assert.equal(requests.length, 1);
    assert.equal(s.displayQuote, confirmed);
    assert.equal(s.quote, null);
    waiters.shift()();
    await tick();
    assert.equal(requests.length, 2);
    assert.equal(requests[1].value, "#00001e");
    assert.equal(requests[1].expectedRevision, 1);
    assert.equal(quotes.length, 0);
    waiters.shift()();
    await s.settle();
    await tick();
    assert.equal(s.result.state.answers.b, "#00001e");
    assert.equal(s.pending, 0);
    assert.equal(quotes.length, 1);
    assert.equal(quotes[0].state.revision, 2);
    assert.ok(frames.slice(3).every((f) => f.price));
    const n = requests.length;
    await s.selectColor("b", "#00001e", true);
    assert.equal(requests.length, n);
    assert.equal(quotes.length, 1);
});
test("continuous input defers Quote until quiet or final change while Visual remains confirmed", async () => {
    let quotes = 0,
        applies = 0;
    const revisions = [];
    const s = createSession({
        send: async (method, body) => {
            if (method === "initial") return { result: result() };
            if (method === "quote") {
                quotes++;
                return {
                    quote: {
                        modelDigest: "digest",
                        revision: body.quote.state.revision,
                        focus: { question: { id: "b" } },
                    },
                };
            }
            applies++;
            return {
                result: result(body.state.revision + 1, {
                    b: body.event.value,
                }),
            };
        },
        changed: (s) => revisions.push(s.result?.state.revision),
        failed: (e) => {
            throw e;
        },
        save: () => {},
    });
    await s.restore(null);
    await tick();
    quotes = 0;
    await s.selectColor("b", "#112233", false);
    await s.selectColor("b", "#445566", false);
    assert.equal(quotes, 0);
    assert.ok(revisions.includes(2));
    await s.selectColor("b", "#445566", true);
    await tick();
    assert.equal(applies, 2);
    assert.equal(quotes, 1);
});
test("replaced color keeps other actions ordered and a failed Apply cannot lose the final color", async () => {
    const calls = [],
        waiters = [];
    const errors = [];
    const s = createSession({
        send: async (method, body) => {
            if (method === "initial") return { result: result() };
            if (method === "quote") return new Promise(() => {});
            calls.push(body.event);
            return new Promise((resolve, reject) =>
                waiters.push({
                    resolve: () =>
                        resolve({
                            result: result(body.state.revision + 1, {
                                ...body.state.answers,
                                [body.event.questionId]: body.event.value,
                            }),
                        }),
                    reject,
                }),
            );
        },
        changed: () => {},
        failed: (e) => errors.push(e.message),
        save: () => {},
    });
    await s.restore(null);
    s.selectColor("b", "#111111");
    s.selectColor("b", "#222222");
    s.select("a", "other");
    s.selectColor("b", "#333333");
    waiters.shift().reject(new Error("temporary failure"));
    await tick();
    assert.equal(calls[1].questionId, "a");
    waiters.shift().resolve();
    await tick();
    assert.equal(calls[2].value, "#333333");
    assert.equal(calls[2].expectedRevision, 1);
    waiters.shift().resolve();
    await s.settle();
    assert.deepEqual(errors, ["temporary failure"]);
    assert.equal(s.result.state.answers.b, "#333333");
    assert.equal(s.pending, 0);
});
