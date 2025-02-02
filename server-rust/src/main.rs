// 引入必要的外部依赖
use anyhow::Result;
use axum::extract::DefaultBodyLimit;
use axum::{routing::post, Json, Router};
use chrono::{Datelike, Local};
use jieba_rs::Jieba; // 引入结巴分词库
use once_cell::sync::Lazy;
use rusqlite::{params, Connection};
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};

// 创建全局共享的 Jieba 实例
static JIEBA: Lazy<Jieba> = Lazy::new(|| {
    println!("Initializing Jieba instance...");
    Jieba::new()
});

#[derive(Deserialize)]
struct PageData {
    content: String,
}

#[derive(Serialize)]
struct ApiResponse {
    success: bool,
    path: Option<String>,
    error: Option<String>,
}

/// 处理页面保存的主要函数
/// 接收页面数据，处理并存储到SQLite数据库中
/// 返回处理结果的JSON响应
async fn save_page(Json(data): Json<PageData>) -> Json<ApiResponse> {
    // 获取当前时间，用于生成数据库文件名
    let now = Local::now();
    let year = now.year();
    let month = now.month();
    // 构造数据库文件路径，按年月分表存储
    let db_path = format!("data/{}_{:02}_pages.db", year, month);

    // 确保data目录存在，如果不存在则创建
    if let Err(e) = std::fs::create_dir_all("data") {
        return Json(ApiResponse {
            success: false,
            path: None,
            error: Some(format!("Failed to create data directory: {}", e)),
        });
    }

    // 使用闭包处理数据库操作，确保资源正确释放
    let result = (|| {
        // 检查数据库文件是否存在
        let db_exists = std::path::Path::new(&db_path).exists();
        if !db_exists {
            // 数据库文件不存在时，先创建空文件
            std::fs::File::create(&db_path).unwrap();
        }

        // 建立数据库连接
        let conn = Connection::open(&db_path)?;

        // 仅在新建数据库时创建表结构
        if !db_exists {
            conn.execute(
                // 创建页面存储表，包含URL、标题、HTML内容、摘要等字段
                "CREATE TABLE IF NOT EXISTS pages (
                    id INTEGER PRIMARY KEY,
                    url TEXT NOT NULL,
                    title TEXT NOT NULL,
                    html TEXT NOT NULL,
                    summary TEXT NOT NULL,
                    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
                )",
                [],
            )?;
        }

        // 解析content中的JSON数据
        let page_content: serde_json::Value = serde_json::from_str(&data.content).unwrap();

        // 预处理HTML文本，避免重复处理
        let html_text = page_content["html"].as_str().unwrap_or("");
        let clean_text = strip_html_tags(html_text).unwrap().trim().to_string();

        // 使用全局 JIEBA 实例
        let summary = extract_summary(&clean_text, 2, &JIEBA).join("");

        // 插入数据
        conn.execute(
            "INSERT INTO pages (url, title, html, summary) VALUES (?1, ?2, ?3, ?4)",
            params![
                page_content["url"].as_str().unwrap_or(""),
                page_content["title"].as_str().unwrap_or(""),
                html_text,
                summary,
            ],
        )?;

        Ok::<_, rusqlite::Error>(db_path)
    })();

    match result {
        Ok(path) => Json(ApiResponse {
            success: true,
            path: Some(path),
            error: None,
        }),
        Err(e) => Json(ApiResponse {
            success: false,
            path: None,
            error: Some(format!("Operation failed: {}", e)),
        }),
    }
}

/// 为句子打分（包含归一化处理）
/// 基于TF（词频）方法计算每个句子的重要性分数
/// 并进行归一化处理，使分数落在0-1之间
fn score_sentences<'a>(
    sentences: &'a [(&str, usize)],
    word_scores: &HashMap<String, usize>,
    jieba: &Jieba,
) -> Vec<(&'a str, usize, f64)> {
    // 预分配内存以提高性能
    let mut scores = Vec::with_capacity(sentences.len());

    // 遍历每个句子，计算其得分
    for &(sentence, idx) in sentences {
        // 对句子进行分词
        let words = jieba.cut(sentence, false);
        // 计算句子得分：句子中所有词的权重之和
        let score: usize = words
            .iter()
            .map(|word| word_scores.get(*word).cloned().unwrap_or(0))
            .sum();
        scores.push((sentence, idx, score));
    }

    // 找出最大分数，用于归一化处理
    let max_score = scores.iter().map(|(_, _, score)| *score).max().unwrap_or(1);

    // 归一化处理：将所有分数映射到0-1范围
    scores
        .into_iter()
        .map(|(sentence, idx, score)| {
            (
                sentence,
                idx,
                if max_score > 0 {
                    score as f64 / max_score as f64
                } else {
                    0.0
                },
            )
        })
        .collect()
}

/// 提取文本摘要
/// 使用TF算法和句子位置信息提取最重要的句子作为摘要
fn extract_summary(text: &str, num_sentences: usize, jieba: &Jieba) -> Vec<String> {
    // 处理空文本的情况
    if text.is_empty() {
        return Vec::new();
    }

    // 分句处理：按标点符号切分，并保留句子的原始位置信息
    let sentences: Vec<(&str, usize)> = text
        .split(|c| c == '。' || c == '！' || c == '？')
        .enumerate()
        .filter(|(_, s)| !s.trim().is_empty()) // 过滤空句子
        .map(|(i, s)| (s.trim(), i)) // 保存句子位置信息
        .collect();

    if sentences.is_empty() {
        return Vec::new();
    }

    // 计算词频
    let mut word_scores = HashMap::with_capacity(1000); // 预分配足够容量
    for word in jieba.cut(text, false) {
        // 对整个文本进行分词
        *word_scores.entry(word.to_string()).or_insert(0) += 1; // 统计词频
    }

    // 对所有句子进行评分
    let mut scored_sentences = score_sentences(&sentences, &word_scores, jieba);

    // 排序：优先按分数降序，分数相同时按原文位置排序
    scored_sentences.sort_by(|a, b| {
        b.2.partial_cmp(&a.2)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.1.cmp(&b.1))
    });

    // 选取指定数量的高分句子
    scored_sentences.truncate(num_sentences);

    // 返回结果
    scored_sentences
        .into_iter()
        .map(|(sentence, _, _)| sentence.to_string())
        .collect()
}

/// HTML文本清理函数
/// 智能提取HTML中的主要文本内容，去除无关的导航、广告等干扰内容
pub fn strip_html_tags(html: &str) -> Result<String> {
    // 解析HTML文档为DOM树
    let document = Html::parse_document(html);

    // 定义需要排除的干扰元素选择器
    let exclude_selector = Selector::parse("script, style, nav, header, footer, #header, #footer, .header, .footer, .nav, .menu, .sidebar, .advertisement, .ad, iframe, noscript").unwrap();

    // 定义主要内容区域的选择器
    let content_selector = Selector::parse(
        "article, .article, .content, .main, main, .post, .post-content, p, h1, h2, h3, h4, h5, h6",
    )
    .unwrap();

    // 第一步：尝试提取主要内容区域的文本
    let mut content: Vec<String> = document
        .select(&content_selector)
        .filter(|element| {
            // 过滤掉位于干扰元素内的内容
            !element.ancestors().any(|ancestor| {
                if let Some(element_ref) = ElementRef::wrap(ancestor) {
                    exclude_selector.matches(&element_ref)
                } else {
                    false
                }
            })
        })
        .flat_map(|element| element.text()) // 提取文本内容
        .map(|text| text.trim().to_string()) // 清理空白字符
        .filter(|text| !text.is_empty()) // 过滤空文本
        .collect();

    // 如果主要内容为空，回退到提取body标签内容
    if content.is_empty() {
        let body_selector = Selector::parse("body").unwrap();
        content = document
            .select(&body_selector)
            .filter(|element| {
                !element.descendants().any(|descendant| {
                    if let Some(element_ref) = ElementRef::wrap(descendant) {
                        exclude_selector.matches(&element_ref)
                    } else {
                        false
                    }
                })
            })
            .flat_map(|element| element.text())
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty())
            .collect();
    }

    // 最终文本清理：合并空格、去除多余空白
    let cleaned_text = content
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    Ok(cleaned_text)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 配置CORS策略，允许跨域请求
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // 配置路由
    let app = Router::new()
        .route("/save", post(save_page)) // 注册保存页面的处理函数
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024)) // 10 MB https://docs.rs/axum/latest/axum/extract/struct.DefaultBodyLimit.html
        .layer(cors); // 应用CORS中间件

    // 绑定服务器地址
    let addr = SocketAddr::from(([0, 0, 0, 0], 3020));
    println!("Server running on http://{}", addr);

    // 启动HTTP服务
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
