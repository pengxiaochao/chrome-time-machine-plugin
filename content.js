/**
 * 将页面数据保存到文件
 * @param {Object} pageData - 包含页面URL、标题和HTML内容的对象
 */
function saveToFile(pageData) {
    // 生成文件名和目录路径
    const currentDate = new Date();
    const dateStr = currentDate.toISOString().split('T')[0];  // 获取当前日期作为目录名
    const timestamp = Date.now();  // 使用时间戳确保文件名唯一
    const hostname = new URL(pageData.url).hostname;
    const filename = `${hostname}-${timestamp}.json`;
    const dirPath = dateStr;

    // 发送POST请求到服务器保存文件
    fetch(config.serverUrl, {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json',
        },
        body: JSON.stringify({
            content: JSON.stringify(pageData, null, 2),
            dirPath: dirPath,
            filename: filename
        })
    })
    .then(response => response.json())
    .then(result => {
        if (result.success) {
            console.log('文件保存成功:', result.path);
        } else {
            console.error('文件保存失败:', result.error);
        }
    })
    .catch(error => console.error('保存文件时发生错误:', error));
}

// 监听来自background.js的消息
chrome.runtime.onMessage.addListener((request, sender, sendResponse) => {
    if (request.action === "getContent") {
        // 检查当前域名是否在白名单中
        const currentUrl = new URL(request.url);
        const isWhitelisted = whitelist.some(domain => currentUrl.hostname.includes(domain));
        
        if (isWhitelisted) {
            console.log('当前域名在白名单中，跳过发送');
            return;
        }

        // 收集页面数据
        const pageData = {
            url: request.url,
            title: request.title,
            html: document.documentElement.outerHTML
        };

        // 保存到服务器
        saveToFile(pageData);
        // 弹出确认对话框
        // if (confirm(`是否要发送当前页面[${request.title}]吗？`)) {
        //     saveToFile(pageData);
        // }
    }
});
