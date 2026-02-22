// form.js for esp32clock

document.addEventListener("DOMContentLoaded", function () {
    bindForm("esp32msg", handleMsgSubmit);
    bindForm("esp32cfg", handleCfgSubmit);
    bindForm("esp32fw", handleFwSubmit);
    initUptime();
});

function bindForm(name, handler) {
    const form = document.querySelector(`form[name='${name}']`);
    if (!form) return;
    ensureStatusNode(form);
    form.addEventListener("submit", handler);
}

function ensureStatusNode(form) {
    let status = form.querySelector(".form-status");
    if (status) return status;

    status = document.createElement("div");
    status.className = "form-status";
    status.setAttribute("role", "status");
    status.setAttribute("aria-live", "polite");
    status.hidden = true;
    form.appendChild(status);
    return status;
}

function setFormStatus(form, kind, message) {
    const status = ensureStatusNode(form);
    status.hidden = !message;
    status.className = `form-status${kind ? ` is-${kind}` : ""}`;
    status.textContent = message || "";
}

function setFormBusy(form, busy, busyLabel) {
    const submit = form.querySelector("input[type='submit']");
    if (!submit) return;

    if (busy) {
        if (!submit.dataset.label) submit.dataset.label = submit.value;
        submit.disabled = true;
        submit.value = busyLabel || "Working...";
    } else {
        submit.disabled = false;
        if (submit.dataset.label) submit.value = submit.dataset.label;
    }
}

async function fetchJsonOrError(url, options) {
    const response = await fetch(url, options);
    const contentType = response.headers.get("content-type") || "";
    let payload;

    if (contentType.includes("application/json")) {
        payload = await response.json();
    } else {
        const text = await response.text();
        payload = {message: text};
    }

    const statusError = payload && payload.status === "error";
    const okFalse = payload && payload.ok === false;
    if (!response.ok || statusError || okFalse) {
        throw new Error((payload && payload.message) || `Request failed (${response.status})`);
    }
    return payload;
}

async function updateUptime() {
    const node = document.getElementById("uptime");
    if (!node) return;

    try {
        const response = await fetch("/uptime");
        const json = await response.json();
        node.textContent = `Uptime: ${json.uptime} s`;
    } catch (_error) {
        node.textContent = "Uptime unavailable";
    }
}

function initUptime() {
    if (!document.getElementById("uptime")) return;
    updateUptime();
    window.setInterval(updateUptime, 30e3);
}

const handleMsgSubmit = async (event) => {
    event.preventDefault();
    const form = event.currentTarget;
    const url = form.action;

    setFormBusy(form, true, "Sending...");
    setFormStatus(form, "busy", "Sending message...");
    try {
        const formData = new FormData(form);
        const responseData = await postMsgDataAsJson({url, formData});
        console.log({responseData});
        setFormStatus(form, "ok", responseData.message || "Message sent");
        const input = form.querySelector("input[name='msg']");
        if (input) input.value = "";
    } catch (error) {
        console.error(error);
        setFormStatus(form, "error", error.message || "Message send failed");
    } finally {
        setFormBusy(form, false);
    }
};

const handleCfgSubmit = async (event) => {
    event.preventDefault();
    const form = event.currentTarget;
    const url = form.action;

    setFormBusy(form, true, "Saving...");
    setFormStatus(form, "busy", "Saving config...");
    try {
        const formData = new FormData(form);
        const responseData = await postCfgDataAsJson({url, formData});
        console.log({responseData});
        setFormStatus(form, "ok", responseData.message || "Config saved");
    } catch (error) {
        console.error(error);
        setFormStatus(form, "error", error.message || "Config save failed");
    } finally {
        setFormBusy(form, false);
    }
};

const handleFwSubmit = async (event) => {
    event.preventDefault();
    const form = event.currentTarget;
    const url = form.action;

    if (!window.confirm("Start firmware update now? The device will reboot if the update succeeds.")) {
        return;
    }

    setFormBusy(form, true, "Updating...");
    setFormStatus(form, "busy", "Downloading and flashing firmware...");
    try {
        const formData = new FormData(form);
        const responseData = await postFwForm({url, formData});
        console.log({responseData});
        setFormStatus(form, "ok", responseData.message || "Firmware update started");
    } catch (error) {
        console.error(error);
        setFormStatus(form, "error", error.message || "Firmware update failed");
    } finally {
        setFormBusy(form, false);
    }
};

const postMsgDataAsJson = async ({url, formData}) => {
    const formObj = Object.fromEntries(formData.entries());

    return fetchJsonOrError(url, {
        method: "POST",
        mode: "cors",
        keepalive: false,
        headers: {"Accept": "application/json", "Content-Type": "application/json"},
        body: JSON.stringify(formObj)
    });
};

const postCfgDataAsJson = async ({url, formData}) => {
    let formObj = Object.fromEntries(formData.entries());
    // convert integers
    formObj.port = parseInt(formObj.port);
    formObj.v4mask = parseInt(formObj.v4mask);
    formObj.led_intensity_day = parseInt(formObj.led_intensity_day);
    formObj.led_intensity_night = parseInt(formObj.led_intensity_night);
    // convert booleans
    formObj.wifi_wpa2ent = (formObj.wifi_wpa2ent === "on");
    formObj.v4dhcp = (formObj.v4dhcp === "on");
    formObj.mqtt_enable = (formObj.mqtt_enable === "on");
    formObj.sensor_enable = (formObj.sensor_enable === "on");
    formObj.display_shutoff_enable = (formObj.display_shutoff_enable === "on");
    // convert floats
    formObj.lat = parseFloat(formObj.lat);
    formObj.lon = parseFloat(formObj.lon);

    return fetchJsonOrError(url, {
        method: "POST",
        mode: "cors",
        keepalive: false,
        headers: {"Accept": "application/json", "Content-Type": "application/json"},
        body: JSON.stringify(formObj)
    });
};

const postFwForm = async ({url, formData}) => {
    const params = new URLSearchParams(formData);
    return fetchJsonOrError(url, {
        method: "POST",
        body: params
    });
};

// EOF
