from transformers import AutoTokenizer, AutoModelForTokenClassification
from transformers import pipeline
from langdetect import detect

def get_model_by_language(language):
    MODEL_NAMES = {
        "en": "dbmdz/bert-large-cased-finetuned-conll03-english",
        "fr": "Jean-Baptiste/camembert-ner"
    }

    model_name = MODEL_NAMES[language]

    tokenizer = AutoTokenizer.from_pretrained(model_name)
    model = AutoModelForTokenClassification.from_pretrained(model_name)
    nlp = pipeline("ner", model=model, tokenizer=tokenizer, aggregation_strategy="simple", device="cuda")
    return nlp



def scan_text(text):
    language = detect(text)

    model = get_model_by_language(language)  # load english model by default

    doc = model(text)
    return doc, language

class TextAnalysisReport:
    language: str
    organisations: list[str]
    persons: list[str]
    def __init__(self, language, organisations, persons):
        self.language = language
        self.organisations = organisations
        self.persons = persons

def analyze_text(text):
    results, language = scan_text(text)

    persons = set()
    organisations = set()

    for entity in results:
        print(f"{entity['word']}: {entity['entity_group']} ({entity['score']:.3f})")
        label = entity['entity_group']
        word = entity['word']
        if label == "PER":
            persons.add(word)
        elif label == "ORG":
            organisations.add(word)

    return TextAnalysisReport(language, list(organisations), list(persons))

