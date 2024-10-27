

from flask import Flask, request

from ner import scan_text
import json

from ner import analyze_text
from ocr import extract_text_from_image

app = Flask(__name__)

@app.post('/ner')
def ner():
    text = request.data.decode('utf-8')
    # result = scan_text(text)
    result = analyze_text(text)
    return json.dumps(result.__dict__)

@app.post('/ocr')
def ocr():
    urls = request.data.decode('utf-8').splitlines()

    print("urls:", request.data.decode('utf-8'))

    texts = {}

    for url in urls:
        texts[url] = extract_text_from_image(url)

    return json.dumps(texts)

app.run(debug=True, port=8080, host='0.0.0.0')