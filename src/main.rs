use chrono::Local;
use clap::Parser;

use reqwest;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{env, error::Error, fs, path::Path};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Name of the item to insert
    #[arg(short, long, value_name = "Name")]
    name: String,

    /// Optional private flag.  
    /// If -p given, "Private?" = Private else, "Private?"= Work
    #[arg(short, long)]
    private: bool,

    /// Add task with today's date
    #[arg(short, long)]
    today: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct Credentials {
    database_id: String,
    secret: String,
}

/**
 * GTDを行う時に、パッとnotionにアイデアやtodoを投稿する為のプログラム
 * inbox -n todo とかで、notionのデータベースに"名前"がtodoのデータが作成される。
 * credential.jsonを指示されたところに作る事。
 */
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let private_status = if args.private { "Private" } else { "Work" };

    // 認証情報ファイルがあるかチェック
    let current_exe_path = env::current_exe()?;
    let current_dir = current_exe_path.parent().unwrap_or_else(|| Path::new(""));

    let credential_path = current_dir.join("credential.json");

    let credentials = match fs::read_to_string(&credential_path) {
        Ok(contents) => serde_json::from_str::<Credentials>(&contents)?,
        Err(_) => {
            println!("'credential.json' not found in '{}'. Please create it in this directory with the following format:\n{{\n    \"database_id\": \"your-database-id\",\n    \"secret\": \"your-integration-secret\"\n}}", current_dir.display());
            return Err("Missing credentials file".into());
        }
    };

    let database_id = credentials.database_id;
    let secret = credentials.secret;

    let mut properties = serde_json::Map::new();
    properties.insert(
        "名前".to_string(),
        json!({
            "title": [
                { "text": { "content": args.name } }
            ]
        }),
    );
    properties.insert(
        "Private?".to_string(),
        json!({
            "select": { "name": private_status }
        }),
    );

    // Add today's date if `-t` is used
    if args.today {
        let today = Local::today().format("%Y-%m-%d").to_string();
        properties.insert(
            "実施予定日".to_string(),
            json!({
                "date": { "start": today }
            }),
        );
    }

    let json_payload = json!({
        "parent": { "database_id": database_id },
        "properties": properties
    });

    let client = reqwest::Client::new();

    let response = client
        .post("https://api.notion.com/v1/pages")
        .header("Authorization", format!("Bearer {}", secret))
        .header("Notion-Version", "2021-08-16")
        .json(&json_payload)
        .send()
        .await?;

    // let _response_json: Value = response.json().await?;
    // println!("successed!");

    if response.status().is_success() {
        let response_json: Value = response.json().await?;
        println!("Page successfully created: {:?}", response_json);
    } else {
        eprintln!("Failed to create page. Status: {:?}", response.status());
        if let Ok(response_text) = response.text().await {
            eprintln!("Response error: {}", response_text);
        }
        return Err("API request failed".into());
    }

    Ok(())
}
