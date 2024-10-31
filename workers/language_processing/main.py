

from flask import Flask, request

import json

from llm import analyze_text
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
def text_analysis():
    return analyze_text(request.data.decode('utf-8')), {"Content-Type": "application/text"}

app.run(debug=True, port=8080, host='0.0.0.0')