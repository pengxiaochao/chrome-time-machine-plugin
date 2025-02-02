// 白名单域名列表，这些域名的页面内容不会被收集
const whitelist = [
  "localhost", // 本地开发环境
  "127.0.0.1", // 本地环回地址
  "192.168", // 局域网地址
  "test.com", // 特定网站
];

// 全局配置对象
const config = {
  savePath: "/Users/pengxiaochao/Downloads/page-collector/", // 本地保存路径
  serverUrl: "http://127.0.0.1:3020/save", // API服务器地址
};
