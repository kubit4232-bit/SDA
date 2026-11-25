// SDA Chrome Extension — content script
// Перехватывает клики по ссылкам на странице и отправляет в background

(function () {
    console.log("[SDA] Content script loaded");

    function handleLinkClick(event) {
        const link = event.target.closest("a[href]");
        if (!link) return;

        const url = link.href;
        if (!url || !url.startsWith("http")) return;

        // Отправляем запрос на скачивание в background
        chrome.runtime.sendMessage(
            { type: "sda_download_link", url: url },
            (response) => {
                if (response && response.status === "ok") {
                    // Предотвращаем стандартное поведение браузера (открытие ссылки)
                    event.preventDefault();
                    event.stopPropagation();
                    return false;
                }
            }
        );
    }

    // Делегирование кликов — ловим все клики по ссылкам на странице
    document.addEventListener("click", handleLinkClick, true);
})();
