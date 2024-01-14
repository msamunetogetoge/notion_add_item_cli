from flask import Flask, request, render_template
import requests
import os
from dotenv import load_dotenv

load_dotenv()  # take environment variables from .env.

app = Flask(__name__)

@app.route('/', methods=['GET', 'POST'])
def index():
    if request.method == 'POST':
        name = request.form.get('name')
        private = 'Private' if 'private' in request.form else 'Work'

        create_notion_page(name, private)
        return render_template('success.html')

    return render_template('index.html')

def create_notion_page(name, private_status):
    database_id = os.getenv('DATABASE_ID')
    secret = os.getenv('SECRET')

    headers = {
        "Authorization": f"Bearer {secret}",
        "Notion-Version": "2021-08-16"
    }

    data = {
        "parent": { "database_id": database_id },
        "properties": {
            "名前": { "title": [{ "text": { "content": name }}]},
            "Private?": { "select": { "name": private_status }}
        }
    }

    response = requests.post("https://api.notion.com/v1/pages", headers=headers, json=data)
    return response.json()

if __name__ == '__main__':
    app.run(debug=True)
