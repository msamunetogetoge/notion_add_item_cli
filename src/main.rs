use chrono::Local;
use clap::Parser;

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
        let today = Local::now().format("%Y-%m-%d").to_string();
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
        .header("Notion-Version", "2022-06-28")
        .json(&json_payload)
        .send()
        .await?;

    if response.status().is_success() {
        println!("Success!");
        let _response_json: Value = response.json().await?;
        // Query the database
        let query_results = query_notion_database(&client, &secret, &database_id).await?;

        // Process and display the counts
        let summary = summarize_tasks(&query_results);
        print!("{}", summary)
    } else {
        eprintln!("Failed to create page. Status: {:?}", response.status());
        if let Ok(response_text) = response.text().await {
            eprintln!("Response error: {}", response_text);
        }
        return Err("API request failed".into());
    }

    Ok(())
}

async fn query_notion_database(
    client: &reqwest::Client,
    secret: &str,
    database_id: &str,
) -> Result<Value, Box<dyn Error>> {
    let today = Local::now().format("%Y-%m-%d").to_string();

    // let query = json!({
    //     "filter": {
    //         "or": [
    //             {
    //                 "and": [
    //                     {
    //                         "or": [
    //                             {
    //                                 "property": "実施予定日",
    //                                 "date": {
    //                                     "equals": today
    //                                 }
    //                             },
    //                             {
    //                                 "property": "実施予定日",
    //                                 "date": {
    //                                     "is_empty": true
    //                                 }
    //                             }
    //                         ]
    //                     },
    //                     {
    //                         "or": [
    //                             {
    //                                 "property": "進行中？",
    //                                 "status": {
    //                                     "equals": "進行中"
    //                                 }
    //                             },
    //                             {
    //                                 "property": "進行中？",
    //                                 "status": {
    //                                     "is_empty": true
    //                                 }
    //                             }
    //                         ]
    //                     }
    //                 ]
    //             },
    //             {
    //                 "or": [
    //                     {
    //                         "property": "タスク種別",
    //                         "select": {
    //                             "equals": "次にとるべき行動リスト"
    //                         }
    //                     },
    //                     {
    //                         "property": "タスク種別",
    //                         "select": {
    //                             "is_empty": true
    //                         }
    //                     }
    //                 ]
    //             }
    //         ]
    //     }
    // });

    let query = json!({
        "filter": {
            "and":[
                {"or": [
                    {
                    "property": "実施予定日",
                    "date": {
                        "equals": today
                    }
                    },
                    {
                        "property": "実施予定日",
                        "date": {
                            "is_empty": true
                        }
                    },
                    ]
                },
                {
                "property": "進行中？",
                "status":{
                    "does_not_equal": "完了"
                }
                }
            ],
        }
    });

    let query_param = [
        ("filter_properties", ":>Jm"), // 進行中？
        ("filter_properties", "`|qV"), // 実施予定日
        ("filter_properties", "e=P>"), // Private?
        ("filter_properties", "MrBR"), // タスク種別
    ];

    let response = client
        .post(format!(
            "https://api.notion.com/v1/databases/{}/query",
            database_id
        ))
        .query(&query_param)
        .header("Authorization", format!("Bearer {}", secret))
        .header("Notion-Version", "2022-06-28")
        .json(&query)
        .send()
        .await?;

    if response.status().is_success() {
        let response_json = response.json::<Value>().await?;
        Ok(response_json)
    } else {
        eprintln!("{}: {}", response.status(), response.text().await?);
        Err("Failed to query database ".into())
    }
}

fn summarize_tasks(query_results: &Value) -> String {
    let mut work_tasks_today = 0;
    let mut private_tasks_today = 0;
    let mut next_actions_tasks = 0;
    let mut no_type_tasks = 0;
    let today = Local::now().format("%Y-%m-%d").to_string();

    if let Some(results) = query_results["results"].as_array() {
        for task in results {
            let task_type = match &task["properties"]["タスク種別"]["select"]["name"] {
                Value::String(s) => s.as_str(),
                _ => "",
            };

            let task_date = match &task["properties"]["実施予定日"]["date"]["start"] {
                Value::String(s) => s.as_str(),
                _ => "",
            };

            let private = match &task["properties"]["Private?"]["select"]["name"] {
                Value::String(s) => s.as_str(),
                _ => "",
            };

            if task_date == today {
                match private {
                    "Work" => work_tasks_today += 1,
                    "Private" => private_tasks_today += 1,
                    _ => {}
                }
            } else {
                match task_type {
                    "▶️ 次に取るべき行動リスト" => next_actions_tasks += 1,
                    "" => no_type_tasks += 1,
                    _ => {}
                }
            }
        }
    }

    format!("今日のタスク: Work {}件, Private {}件\n未定義のタスク: 次にとるべき行動リスト {}件, タスク種別なし {}件",
            work_tasks_today, private_tasks_today, next_actions_tasks, no_type_tasks)
}
