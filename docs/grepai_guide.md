# 🔍 grepai 安裝與操作指南 (Windows)

`grepai` 是一個專為 AI 時代打造的命令列介面 (CLI) 程式碼語意搜尋工具。它允許你用「自然語言」直接搜尋專案內的邏輯，而無需依賴精確的關鍵字比對。

本文檔將引導你如何在 Windows (PowerShell) 環境下，從零開始安裝、初始化與執行 `grepai`。

---

## 1. 📥 安裝 grepai

在你的 **PowerShell 7** 終端機中，執行官方提供的安裝腳本：

```powershell
irm https://raw.githubusercontent.com/yoanbernabeu/grepai/main/install.ps1 | iex
```

> **⚠️ 注意事項：環境變數設定**  
> 安裝腳本預設會將 `grepai.exe` 放在 `C:\Users\<你的使用者名稱>\AppData\Local\Programs\grepai` 目錄下。
> 如果你的終端機遇到 `找不到 grepai 指令` 的錯誤，代表腳本沒有成功將它加入環境變數 `PATH`。
> 
> **手動補救步驟**（新增至永久環境變數）：
> 請在終端機執行以下指令：
> ```powershell
> $newPath = "$env:LOCALAPPDATA\Programs\grepai"
> $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
> [Environment]::SetEnvironmentVariable("Path", $UserPath + ";" + $newPath, "User")
> $env:PATH += ";" + $newPath
> ```
> 完成後輸入 `grepai version`，若能看到版號就代表安裝成功了。

---

## 2. 🤖 準備本地模型 (Ollama)

`grepai` 在執行語意搜尋時需要依賴一個 **Embedding（嵌入）** 模型來理解語意。
最推薦且注重隱私的作法是使用本地的 [Ollama](https://ollama.com/) 服務。

如果你選擇使用本地 Ollama，請確保已安裝 Ollama，並且拉取（Pull）預設的嵌入模型：

```powershell
ollama pull nomic-embed-text
```

---

## 3. ⚙️ 初始化你的專案

進到你要分析的專案目錄下（例如 `mini_bot`）：

```powershell
cd D:\github\chiisen\mini_bot
grepai init
```

執行後會出現互動式設定表單，建議依照以下選擇：
1. **Select embedding provider（選擇嵌入提供者）**：選 `1) ollama`（預設）。
2. **Select storage backend（選擇儲存後端）**：選 `1) gob`（預設，本地檔案形式儲存，適合中小型專案）。

這會在專案內建立一個隱藏的 `.grepai/config.yaml` 設定檔，同時會自動將它加入 `.gitignore` 避免外流。

---

## 4. 👁️ 啟動「索引監聽」 Daemon

為了讓 `grepai` 能夠理解你的程式碼結構，並在你修改程式碼時即時更新索引，你需要開啟一個背景監聽服務。

**請開啟一個「專門讓它一直掛著」的終端機視窗**，在專案目錄下執行：

```powershell
grepai watch
```

- 在第一次掃描時，它會將專案的程式碼切塊，並呼叫 Ollama 產生語意向量。
- 掃描完畢後會顯示 `Watching for changes...`。
- **請將此視窗縮小放著，千萬不要關掉。** 只要你儲存了程式碼的修改，它就會自動在背景幫你更新索引！

---

## 5. 🔍 執行自然語言語意搜尋

當背景的監聽服務運作中時，你可以隨時開啟 **另外一個新的 PowerShell 視窗**（同樣要進入專案目錄），開始你的語意搜尋黑魔法！

例如，當你尋找某段邏輯，卻想不起變數名稱時：

```powershell
grepai search "這個專案怎麼實作載入歷史訊息的？"
```

或是：

```powershell
grepai search "如果 LLM API 失敗時，程式是在哪裡處理錯誤重試的？"
```

它會分析你自然語言的問題，自動找出整個專案中「語意最相近」的程式碼片段。

---

### 💡 核心工作流總結
1. **終端機 A（背景）**：永遠掛著 `grepai watch` 進行即時索引。
2. **終端機 B（前景）**：隨時拿來敲 `grepai search "你想尋找的邏輯描述"` 快速定位程式碼。
