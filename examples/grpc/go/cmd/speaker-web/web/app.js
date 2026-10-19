import { createSession } from "./session.mjs";
const $ = (id) => document.getElementById(id),
    storageKey = "knot-speaker-v1";
const labels = {
    speaker_type: "音箱类型",
    material: "箱体材质",
    panel_color: "面板颜色",
    power: "输出功率",
    extras: "附加功能",
    cable: "接口类型",
};
const values = {
    active: "有源音箱",
    passive: "无源音箱",
    aluminum: "铝合金",
    walnut: "胡桃木",
    "200w": "200W",
    "100w": "100W",
    "50w": "50W",
    led: "LED 灯环",
    bluetooth: "蓝牙模块",
    banana: "香蕉插头",
    spade: "叉形端子",
};
const money = (n, c) =>
    new Intl.NumberFormat("zh-CN", {
        style: "currency",
        currency: c,
        maximumFractionDigits: 2,
    }).format(n / 100);
function priceText(p) {
    if (!p) return "等待报价";
    if (p.status === "no_completion") return "无法完成此配置";
    if (p.status === "unavailable") return "暂无法报价";
    return p.minMinor === p.maxMinor
        ? money(p.minMinor, p.currency)
        : money(p.minMinor, p.currency) + " – " + money(p.maxMinor, p.currency);
}
function text(el, value) {
    if (el.textContent !== value) el.textContent = value;
}
function hidden(el, value) {
    if (el.hidden !== value) el.hidden = value;
}
function attribute(el, name, value) {
    if (el.getAttribute(name) !== value) el.setAttribute(name, value);
}
function style(el, name, value) {
    if (el.style.getPropertyValue(name) !== String(value))
        el.style.setProperty(name, String(value));
}
function toggle(el, name, value) {
    if (el.classList.contains(name) !== value) el.classList.toggle(name, value);
}
function node(tag, value, cls) {
    const e = document.createElement(tag);
    if (value !== undefined) e.textContent = value;
    if (cls) e.className = cls;
    return e;
}
function button(value, action, cls) {
    const e = node("button", value, cls);
    e.type = "button";
    e.onclick = action;
    return e;
}
function failed(e) {
    hidden($("error"), false);
    text($("error"), e.message);
}
async function send(method, body = {}, signal) {
    const response = await fetch("/api/" + method, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
        signal,
    });
    const payload = await response.json();
    text(
        $("raw"),
        JSON.stringify({ method, request: body, response: payload }, null, 2),
    );
    text($("request-info"), method.toUpperCase());
    if (!response.ok || payload.transportError)
        throw new Error(
            "服务调用失败：" + (payload.transportError || response.status),
        );
    if (payload.error)
        throw new Error(
            payload.error.error.code + "：" + payload.error.error.message,
        );
    text($("connection"), "服务已连接");
    return payload;
}
const session = createSession({
    send,
    changed: render,
    failed,
    save: (state) => {
        try {
            localStorage.setItem(storageKey, JSON.stringify(state));
        } catch {}
    },
});
let shown = undefined,
    focusData = null,
    stepKey = null;
function select(q, value, final = true) {
    hidden($("error"), true);
    return q.type === "color"
        ? session.selectColor(q.id, value, final)
        : session.select(q.id, value, q.type !== "single");
}
function renderQuestion(s) {
    const area = $("question");
    if (s.quote) focusData = s.quote.focus;
    if (focusData?.question.id !== s.focus) focusData = null;
    const q =
        focusData?.question ||
        (s.result.snapshot.flow.currentQuestion?.id === s.focus
            ? s.result.snapshot.flow.currentQuestion
            : null);
    const key = q ? `${q.id}:${q.type}` : s.focus;
    if (!q) {
        if (shown !== key) {
            area.replaceChildren(
                node("h2", s.focus ? "读取选项…" : "配置完成"),
            );
            shown = key;
        }
        return;
    }
    if (shown !== key) {
        shown = key;
        area.replaceChildren(node("h2", labels[q.id] || q.label));
        if (q.type === "single" || q.type === "multi") {
            const options = node("div", undefined, "options");
            for (const option of q.options) {
                const el = button(
                    "",
                    () =>
                        select(
                            q,
                            q.type === "multi"
                                ? (state) => {
                                      const old = state.answers[q.id] || [];
                                      return q.options
                                          .filter((o) =>
                                              o.id === option.id
                                                  ? !old.includes(o.id)
                                                  : old.includes(o.id),
                                          )
                                          .map((o) => o.id);
                                  }
                                : option.id,
                        ),
                    "option",
                );
                el.dataset.optionId = option.id;
                const copy = node("span", values[option.id] || option.label);
                copy.append(node("small", "等待报价"));
                el.append(copy, node("span", "", "option-mark"));
                options.append(el);
            }
            area.append(options);
            if (!q.required) {
                const skip = button("跳过此项", () =>
                    session.select(q.id, null, false, "skip_answer"),
                );
                skip.dataset.skip = "true";
                area.append(skip);
            }
        } else if (q.type === "color") {
            const row = node("div", undefined, "color-row"),
                input = node("input"),
                hex = node("span", "选择颜色");
            input.type = "color";
            input.value = s.result.state.answers[q.id] || "#315a79";
            input.setAttribute("aria-label", "面板颜色");
            const submit = (final = true) => {
                text(hex, input.value.toUpperCase());
                select(q, input.value.toUpperCase(), final);
            };
            input.oninput = () => submit(false);
            input.onchange = () => submit(true);
            row.append(input, hex);
            area.append(row, node("p", "等待报价", "candidate-price"));
            const use = button("使用此颜色", () => submit(true));
            use.dataset.useColor = "true";
            area.append(use);
        } else {
            const input = node("input");
            input.type = q.type === "integer" ? "number" : "text";
            input.setAttribute("aria-label", q.label);
            input.onchange = () =>
                select(
                    q,
                    q.type === "integer" ? Number(input.value) : input.value,
                );
            area.append(input);
        }
    }
    const answer = s.result.state.answers[q.id],
        pending = s.quoteStatus !== "settled";
    attribute(area, "aria-busy", String(pending));
    if (q.type === "single" || q.type === "multi") {
        for (const el of area.querySelectorAll("[data-option-id]")) {
            const id = el.dataset.optionId,
                option = q.options.find((o) => o.id === id),
                c = focusData?.candidates.find((c) => c.optionId === id);
            const selected =
                q.type === "multi"
                    ? (answer || []).includes(id)
                    : answer === id;
            toggle(el, "selected", selected);
            attribute(el, "aria-pressed", String(selected));
            text(el.querySelector(".option-mark"), selected ? "✓" : "");
            el.disabled = !(c?.enabled ?? option.enabled) || !!c?.error;
            text(
                el.querySelector("small"),
                !option.enabled
                    ? "当前不可选"
                    : c?.error
                      ? "此选择无效：" + c.error.error.code
                      : "选择后总价：" + priceText(c?.price),
            );
        }
        const skip = area.querySelector("[data-skip]");
        if (skip)
            hidden(
                skip,
                Object.hasOwn(s.result.state.answers, q.id) ||
                    s.result.state.skipped.includes(q.id),
            );
    } else if (q.type === "color") {
        const input = area.querySelector("input");
        // An earlier Apply may confirm while the user is editing a newer color.
        // Keep that draft; the SVG separately follows only confirmed Visual.
        if (document.activeElement !== input && !s.pending) {
            const value = answer || "#315A79";
            if (input.value.toUpperCase() !== value) input.value = value;
            text(area.querySelector(".color-row span"), answer || "选择颜色");
        }
        hidden(area.querySelector("[data-use-color]"), !!answer);
        text(
            area.querySelector(".candidate-price"),
            "选择后总价：" + priceText(s.quote?.price || s.displayQuote?.price),
        );
    } else {
        const input = area.querySelector("input");
        if (document.activeElement !== input) input.value = answer ?? "";
    }
}
function render(s) {
    if (!s.result) return;
    const { state, snapshot } = s.result,
        path = snapshot.flow.path;
    text(
        $("revision"),
        "Revision " + state.revision + (s.pending ? " · 保存中" : ""),
    );
    $("reset").disabled = !!s.pending;
    const targets = Object.fromEntries(
            snapshot.visual.targets.map((t) => [t.id, t.properties]),
        ),
        cabinet = targets["speaker.cabinet"],
        panel = targets["speaker.front_panel"],
        led = targets["speaker.led_ring"];
    toggle($("speaker"), "walnut", cabinet?.assetRef === "cabinet-walnut");
    for (const [layer, target] of [
        ["base-layer", "speaker.base"],
        ["cabinet-layer", "speaker.cabinet"],
        ["panel-layer", "speaker.front_panel"],
    ])
        hidden($(layer), targets[target]?.visible === false);
    style($("speaker"), "--panel-color", panel?.color || "#26272B");
    style($("speaker"), "--led-color", led?.["emission.color"] || "#315A79");
    hidden($("led-layer"), !led?.visible);
    style($("led-layer"), "opacity", led?.["emission.strength"] ?? 1);
    text(
        $("material-label"),
        cabinet?.assetRef === "cabinet-walnut"
            ? "胡桃木"
            : cabinet?.assetRef === "cabinet-aluminum"
              ? "铝合金"
              : "箱体预览",
    );
    const price = s.quote?.price || s.displayQuote?.price,
        pending = s.quoteStatus !== "settled";
    text($("price"), priceText(price));
    text(
        $("price-status"),
        pending
            ? (s.displayQuote ? "上次报价 · " : "") +
                  (s.quoteStatus === "failed" ? "更新失败" : "更新中…")
            : price?.status === "final"
              ? "完整配置总价"
              : "完整配置价格范围",
    );
    text(
        $("price-note"),
        pending
            ? s.displayQuote
                ? "价格与选项正在更新，请以更新后的报价为准。"
                : "正在获取完整配置报价。"
            : price?.status === "unavailable"
              ? "超出精确报价范围，可继续配置后重试。"
              : price?.status === "no_completion"
                ? "请返回修改已有选项。"
                : "",
    );
    const key = JSON.stringify(path);
    if (key !== stepKey) {
        stepKey = key;
        $("steps").replaceChildren(
            ...path.map((id) => {
                const el = button(labels[id] || id, () => s.navigate(id));
                el.dataset.questionId = id;
                return el;
            }),
        );
    }
    for (const el of $("steps").children)
        toggle(el, "active", el.dataset.questionId === s.focus);
    renderQuestion(s);
    const index = path.indexOf(s.focus);
    $("back").disabled = !!s.pending || (index <= 0 && s.focus !== null);
    $("back").onclick = () =>
        s.navigate(s.focus === null ? path.at(-1) : path[index - 1]);
    const handled =
        s.focus === null ||
        Object.hasOwn(state.answers, s.focus) ||
        state.skipped.includes(s.focus);
    $("next").disabled = !!s.pending || !handled || s.focus === null;
    text(
        $("next"),
        index === path.length - 1 && snapshot.flow.status === "complete"
            ? "完成"
            : "下一项",
    );
    $("next").onclick = () => s.navigate(path[index + 1] ?? null);
}
$("reset").onclick = async () => {
    hidden($("error"), true);
    try {
        await session.restore(null);
    } catch (e) {
        failed(e);
    }
};
async function boot() {
    try {
        for (const name of ["base", "cabinet", "panel", "led"]) {
            const response = await fetch(name + ".svg");
            if (!response.ok) throw new Error("无法加载图层");
            const svg = new DOMParser().parseFromString(
                await response.text(),
                "image/svg+xml",
            ).documentElement;
            if (svg.localName !== "svg") throw new Error("无效图层");
            $(name + "-layer").append(document.importNode(svg, true));
        }
        let saved = null;
        const raw = localStorage.getItem(storageKey);
        if (raw) saved = JSON.parse(raw);
        await session.restore(saved);
    } catch (e) {
        failed(e);
        text($("connection"), "恢复失败，请检查诊断或重新开始");
    }
}
boot();
