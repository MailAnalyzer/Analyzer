import pytesseract
import requests
from PIL import Image


def extract_text_from_image(url):
    image_get_response = requests.get(url, stream=True, headers={'User-Agent': 'Mozilla/5.0'})
    image_get_response.raise_for_status()

    image = Image.open("/home/maxime/Downloads/ptn.png").convert("RGB")

    return pytesseract.image_to_string(image, lang='eng')
