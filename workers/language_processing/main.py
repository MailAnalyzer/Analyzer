import asyncio
import json
from threading import Thread

from flask import Flask, request

from llm import llm_loop, submit_job
import json

from ocr import extract_text_from_image

app = Flask(__name__)

@app.post('/ocr')
def ocr():
    urls = request.data.decode('utf-8').splitlines()

    print("urls:", request.data.decode('utf-8'))

    texts = {}

    for url in urls:
        texts[url] = extract_text_from_image(url)

    return json.dumps(texts)

@app.post("/llm/analyze")
async def text_analysis():
    return (await submit_job(request.data.decode('utf-8'))), {"Content-Type": "application/text"}


def initialize():

    def llm_async_loop():
        asyncio.run(llm_loop())

    print("Launching LLM Loop...")
    Thread(target=llm_async_loop, daemon=True).start()


initialize()
print("Starting HTTP Server...")

app.run(debug=True, port=8080, host='0.0.0.0', use_reloader=False)
