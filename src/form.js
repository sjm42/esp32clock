// form.js for esp32clock

document.addEventListener("DOMContentLoaded", function () {
    document.querySelector("form[name='esp32msg']")
        .addEventListener("submit", handleMsgSubmit);
    document.querySelector("form[name='esp32cfg']")
        .addEventListener("submit", handleCfgSubmit);
});

const handleMsgSubmit = async (event) => {
    event.preventDefault();
    const form = event.currentTarget;
    const url = form.action;

    try {
        const formData = new FormData(form);
        const responseData = await postMsgDataAsJson({url, formData});
        console.log({responseData});
    } catch (error) {
        console.error(error);
    }
}

const handleCfgSubmit = async (event) => {
    event.preventDefault();
    const form = event.currentTarget;
    const url = form.action;

    try {
        const formData = new FormData(form);
        const responseData = await postCfgDataAsJson({url, formData});
        console.log({responseData});
    } catch (error) {
        console.error(error);
    }
}


const postMsgDataAsJson = async ({url, formData}) => {
    const formObj = Object.fromEntries(formData.entries());
    const formDataJsonString = JSON.stringify(formObj);

    const fetchOptions = {
        method: "POST", mode: 'cors', keepalive: false,
        headers: {'Accept': 'application/json', 'Content-Type': 'application/json'},
        body: formDataJsonString
    };
    const response = await fetch(url, fetchOptions);

    if (!response.ok) {
        const errorMessage = await response.text();
        throw new Error(errorMessage);
    }
    return response.json();
}

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
    //convert floats
    formObj.lat = parseFloat(formObj.lat);
    formObj.lon = parseFloat(formObj.lon);
    // serialize to JSON
    const formDataJsonString = JSON.stringify(formObj);

    const fetchOptions = {
        method: "POST", mode: 'cors', keepalive: false,
        headers: {'Accept': 'application/json', 'Content-Type': 'application/json'},
        body: formDataJsonString
    };
    const response = await fetch(url, fetchOptions);

    if (!response.ok) {
        const errorMessage = await response.text();
        throw new Error(errorMessage);
    }

    return response.json();
}

// EOF
