// SDA Chrome Extension — background service worker
// Связь с SDA Core через Native Messaging (com.tonec.idm analog → com.sda.agent)

const NATIVE_HOST_NAME = "com.sda.agent";
const CONTEXT_MENU_ID = "sda_download_link";

console.log("[SDA] Background service worker started");

// Создаём пункт контекстного меню для ссылок
chrome.contextMenus.create({
    id: CONTEXT_MENU_ID,
    title: "Download with SDA",
    contexts: ["link"],
});

// Обработчик клика по пункту меню
chrome.contextMenus.onClicked.addListener((info, tab) => {
    if (info.menuItemId === CONTEXT_MENU_ID && info.linkUrl) {
        sendDownloadRequest({
            url: info.linkUrl,
            referer: info.pageUrl || tab.url,
            tabId: tab.id,
        });
    }
});

// Отправка запроса на скачивание в SDA Core через Native Messaging
function sendDownloadRequest(payload) {
    const message = {
        type: "DownloadRequest",
        payload: {
            url: payload.url,
            referer: payload.referer || null,
            cookies: null,
            save_path: null,
            username: null,
            password: null,
        },
    };

    console.log("[SDA] Sending download request:", message.url);

    try {
        chrome.runtime.sendNativeMessage(NATIVE_HOST_NAME, message, (response) => {
            if (chrome.runtime.lastError) {
                console.error("[SDA] Native messaging error:", chrome.runtime.lastError.message);
                // Fallback: показать уведомление, что SDA не запущен
                chrome.notifications.create({
                    type: "basic",
                    iconUrl: "icons/icon48.png",
                    title: "SDA не запущен",
                    message: "Запустите SDA Core, чтобы скачивать файлы через браузер.",
                });
            } else {
                console.log("[SDA] Native messaging response:", response);
            }
        });
    } catch (e) {
        console.error("[SDA] Exception sending native message:", e);
    }
}

// Также перехватываем ссылки из content-скрипта (сообщение от страницы)
chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
    if (message.type === "sda_download_link") {
        sendDownloadRequest({
            url: message.url,
            referer: sender.tab?.url || null,
            tabId: sender.tab?.id || null,
        });
        sendResponse({ status: "ok" });
    }
    return true;
});
