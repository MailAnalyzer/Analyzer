import asyncio
from asyncio import Queue, AbstractEventLoop
from typing import Generator

from llama_cpp import Llama, LlamaGrammar

# llama = Llama("./Mistral-7B-Instruct-v0.3.Q8_0.gguf", n_gpu_layers=200, n_ctx=8192, gpu_reserve_mem=512, n_batch=1024, n_threads=20, n_threads_batch=20)
LLM = Llama("./Mistral-7B-Instruct-v0.3.Q8_0.gguf", n_gpu_layers=200, n_ctx=8192, n_batch=512, n_threads=20,
            n_threads_batch=20)

SCHEMA = r'''
root ::= "summary:" string "\n" "entities:" Entitylist
Information ::= "{"   ws   "\"type\":"   ws   ("\"profession\"" | "\"address\"" | "\"service\"" | "\"product\"" )   ","   ws   "\"value\":"   ws   string   "}"
Informationlist ::= "[]" | "["   ws   Information   (","   ws   Information)*   "]"
Entity ::= "{"   ws   "\"type\":"   ws   (("\"enterprise\", \"products and services\":" stringlist) | "\"person\"" | "\"activity\"") ","   ws   "\"name\":"   ws   name   ","   ws   "\"information\":"   ws   Informationlist   "}"
Entitylist ::= "[]" | "["   ws   Entity   (","   ws   Entity)*   "]"
Reply ::= "{"   ws   "\"summary\":"   ws   string   ","   ws   "\"entities\":"   ws   Entitylist   "}"
string ::= "\""   ([^"]+)   "\""
name ::= "\""   ([a-zA-Z ]+)   "\""
boolean ::= "true" | "false"
ws ::= [ \t\n]*
number ::= [0-9]+   "."?   [0-9]*
stringlist ::= "["   ws   "]" | "["   ws   string   (","   ws   string)*   ws   "]"
numberlist ::= "["   ws   "]" | "["   ws   string   (","   ws   number)*   ws   "]"
'''

GRAMMAR = LlamaGrammar.from_string(grammar=SCHEMA, verbose=False)

LLM_LOOP: AbstractEventLoop = None
LLM_JOB_QUEUE = Queue(150)


class Job:
    text: str
    loop: AbstractEventLoop
    tokens: Queue[str | None]

    def __init__(self, text: str, loop: AbstractEventLoop, tokens_queue: Queue[str | None]):
        self.text = text
        self.loop = loop
        self.tokens = tokens_queue


async def llm_loop():
    global LLM_LOOP
    LLM_LOOP = asyncio.get_event_loop()
    while True:
        job = await LLM_JOB_QUEUE.get()
        try:
            await analyze_text(job)
        except Exception as e:
            print("Error in LLM Loop :", e)


async def submit_job(text: str) -> Generator[str, str, None]:
    loop = asyncio.new_event_loop()
    token_queue = Queue()

    def generator():
        while True:
            token = loop.run_until_complete(token_queue.get())
            if token is None:
                break
            yield token

    j = Job(text, loop, token_queue)
    asyncio.run_coroutine_threadsafe(LLM_JOB_QUEUE.put(j), LLM_LOOP)

    return generator()


async def analyze_text(job: Job):
    prompt = f"""
        You are an email analyst assistant. You are used to understand a text in its details, and will formulate a detailed reply that will be composed of two detailed parts :
        First part (summary), you will provide a detailed summary of the email. You will tell what is its purpose, what the sender aims for, and try to explain if the mail tries to get in touch with the receiver, sell/propose services, is a regular conversation or a notification etc. The summary is detailed but cant exceed more than 7 sentences, minimum 5 sentences (you can put multiple information per sentences). Please add a § char at the end of your sentences.
        Second part (entities), you will collect all the person names and enterprises mentioned ONLY (not products or services). You cant repeat yourself, be aware of synonyms. For each of enterprises and person names, you'll provide a list of ALL bound information (even the most implicit ones) such as their addresses, professions, products and services.
        Here is the text :
        {job.text}
    """
    response = LLM.create_completion(prompt, grammar=GRAMMAR, temperature=0, repeat_penalty=1.1, frequency_penalty=1,
                                     presence_penalty=0, max_tokens=4096, stream=True)
    # response = llama.create_completion("Generate a JSON containing 5 cars associated with their fictive owner : their features, their owners.", grammar=grammar, temperature=0, max_tokens=8192, stream=True)

    for item in response:
        token = item["choices"][0]["text"]
        print(token, end='', flush=True)
        job.loop.call_soon_threadsafe(job.tokens.put_nowait, token)

    job.loop.call_soon_threadsafe(job.tokens.put_nowait, None)
