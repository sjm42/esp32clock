var postCfgDataAsJson = async ({
                                   url, formData
                               }) => {
    const formObj = Object.fromEntries(formData.entries());
    formObj.port = parseInt(formObj.port);
    formObj.v4dhcp = (formObj.v4dhcp === "on");
    formObj.v4mask = parseInt(formObj.v4mask);
    formObj.enable_mqtt = (formObj.enable_mqtt === "on");
    formObj.lat = parseFloat(formObj.lat);
    formObj.lon = parseFloat(formObj.lon);
    const formDataJsonString = JSON.stringify(formObj);

    const fetchOptions = {
        method: "POST", mode: 'cors', keepalive: false, headers: {
            'Accept': 'application/json', 'Content-Type': 'application/json',
        }, body: formDataJsonString,
    };
    const response = await fetch(url, fetchOptions);

    if (!response.ok) {
        const errorMessage = await response.text();
        throw new Error(errorMessage);
    }

    return response.json();
}

var handleCfgSubmit = async (event) => {
    event.preventDefault();
    const form = event.currentTarget;
    const url = form.action;

    try {
        formData = new FormData(form);
        const responseData = await postCfgDataAsJson({
            url, formData
        });
        console.log({
            responseData
        });
    } catch (error) {
        console.error(error);
    }
}

var postMsgDataAsJson = async ({
                                   url, formData
                               }) => {
    const formObj = Object.fromEntries(formData.entries());
    const formDataJsonString = JSON.stringify(formObj);

    const fetchOptions = {
        method: "POST", mode: 'cors', keepalive: false, headers: {
            'Accept': 'application/json', 'Content-Type': 'application/json',
        }, body: formDataJsonString,
    };
    const response = await fetch(url, fetchOptions);

    if (!response.ok) {
        const errorMessage = await response.text();
        throw new Error(errorMessage);
    }

    return response.json();
}

var handleMsgSubmit = async (event) => {
    event.preventDefault();
    const form = event.currentTarget;
    const url = form.action;

    try {
        formData = new FormData(form);
        const responseData = await postMsgDataAsJson({
            url, formData
        });
        console.log({
            responseData
        });
    } catch (error) {
        console.error(error);
    }
}

document.addEventListener("DOMContentLoaded", function () {
    document.querySelector("form[name='esp32cfg']")
        .addEventListener("submit", handleCfgSubmit);
    document.querySelector("form[name='esp32msg']")
        .addEventListener("submit", handleMsgSubmit);
});
