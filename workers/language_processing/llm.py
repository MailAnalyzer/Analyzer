from llama_cpp import Llama, LlamaGrammar

# llama = Llama("./Mistral-7B-Instruct-v0.3.Q8_0.gguf", n_gpu_layers=200, n_ctx=8192, gpu_reserve_mem=512, n_batch=1024, n_threads=20, n_threads_batch=20)
llama = Llama("./Mistral-7B-Instruct-v0.3.Q8_0.gguf", n_gpu_layers=20, n_ctx=8192, n_batch=256, n_threads=20, n_threads_batch=20)

schema = r'''
root ::= "summary:" string "\n" "entities:" Entitylist
Information ::= "{"   ws   "\"type\":"   ws   ("\"profession\"" | "\"address\"" | "\"service\"" | "\"product\"" )   ","   ws   "\"value\":"   ws   string   "}"
Informationlist ::= "[]" | "["   ws   Information   (","   ws   Information)*   "]"
Entity ::= "{"   ws   "\"type\":"   ws   (("\"enterprise\", \"products and services\":" stringlist) | "\"person\"" | "\"activity\"") ","   ws   "\"name\":"   ws   string   ","   ws   "\"information\":"   ws   Informationlist   "}"
Entitylist ::= "[]" | "["   ws   Entity   (","   ws   Entity)*   "]"
Reply ::= "{"   ws   "\"summary\":"   ws   string   ","   ws   "\"entities\":"   ws   Entitylist   "}"
string ::= "\""   ([^"]*)   "\""
boolean ::= "true" | "false"
ws ::= [ \t\n]*
number ::= [0-9]+   "."?   [0-9]*
stringlist ::= "["   ws   "]" | "["   ws   string   (","   ws   string)*   ws   "]"
numberlist ::= "["   ws   "]" | "["   ws   string   (","   ws   number)*   ws   "]"
'''

grammar = LlamaGrammar.from_string(grammar=schema, verbose=False)

def analyze_text(text: str):
    prompt = f"""
        You are an email analyst assistant. You are used to understand a text in its details, and will formulate a detailed reply that will be composed of two detailed parts :
        First part (summary), you will provide a detailed summary of the email. You will tell what is its purpose, what the sender aims for, and try to explain if the mail tries to get in touch with the receiver, sell/propose services, is a regular conversation or a notification etc. The summary is detailed but cant exceed more than 7 sentences, minimum 5 sentences (you can put multiple information per sentences).
        Second part (entities), you will collect all the person names and enterprises mentioned ONLY (not products or services). You cant repeat yourself, be aware of synonyms. For each of enterprises and person names, you'll provide a list of ALL bound information (even the most implicit ones) such as their addresses, professions, products and services.
        Here is the text :
        {text}
    """
    response = llama.create_completion(prompt, grammar=grammar, temperature=0, repeat_penalty=1.1, frequency_penalty=1, presence_penalty=0, max_tokens=4096, stream=True)
    # response = llama.create_completion("Generate a JSON containing 5 cars associated with their fictive owner : their features, their owners.", grammar=grammar, temperature=0, max_tokens=8192, stream=True)

    for item in response:
        token = item["choices"][0]["text"]
        print(token, end='', flush=True)
        yield token


