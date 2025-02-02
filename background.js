// 用于防止重复处理的缓存
const processedUrls = new Map();

// 清理超过5分钟的缓存记录
setInterval(() => {
    const now = Date.now();
    for (const [url, timestamp] of processedUrls.entries()) {
        if (now - timestamp > 5 * 60 * 1000) {
            processedUrls.delete(url);
        }
    }
}, 60 * 1000);

// 统一处理页面变化的函数
function handlePageChange(details) {
    if (details.frameId === 0) {
        // 检查是否最近处理过该URL
        const now = Date.now();
        const lastProcessed = processedUrls.get(details.url);
        if (lastProcessed && (now - lastProcessed < 10000)) {
            return; // 如果10秒内处理过，则跳过
        }
        processedUrls.set(details.url, now);

        setTimeout(() => {
            chrome.tabs.get(details.tabId, (tab) => {
                if (chrome.runtime.lastError) {
                    console.error('获取标签页信息失败:', chrome.runtime.lastError);
                    return;
                }
                
                if (tab.status === 'complete') {
                    chrome.tabs.sendMessage(details.tabId, {
                        action: "getContent",
                        url: tab.url,
                        title: tab.title
                    });
                }
            });
        }, 1000); // 增加延迟到1秒
    }
}

// 添加更多监听器
chrome.webNavigation.onCompleted.addListener(handlePageChange);
chrome.webNavigation.onHistoryStateUpdated.addListener(handlePageChange);
chrome.webNavigation.onReferenceFragmentUpdated.addListener(handlePageChange);

// 监听标签页更新
chrome.tabs.onUpdated.addListener((tabId, changeInfo, tab) => {
    if (changeInfo.status === 'complete') {
        handlePageChange({
            frameId: 0,
            tabId: tabId,
            url: tab.url
        });
    }
});