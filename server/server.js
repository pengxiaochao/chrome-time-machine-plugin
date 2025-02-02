const express = require('express');
const cors = require('cors');
const fs = require('fs').promises;
const path = require('path');
const app = express();

app.use(cors());
app.use(express.json({ limit: '50mb' }));

app.post('/save', async (req, res) => {
    try {
        console.log('Saving file:', req.body);
        const { content, dirPath, filename } = req.body;
        const fullDirPath = path.join(__dirname, 'data', dirPath);
        console.log('Saving file to:', fullDirPath);
        // 确保目录存在
        await fs.mkdir(fullDirPath, { recursive: true });
        
        // 写入文件
        const fullPath = path.join(fullDirPath, filename);
        await fs.writeFile(fullPath, content);
        
        res.json({ success: true, path: fullPath });
    } catch (error) {
        console.error('Error saving file:', error);
        res.status(500).json({ error: error.message });
    }
});

const PORT = 3020;
app.listen(PORT, () => {
    console.log(`Server running on http://localhost:${PORT}`);
});
