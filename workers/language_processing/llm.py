from llama_cpp import Llama, LlamaGrammar

with open("./tests.txt", "r") as f:
    text = ''.join(f.readlines())

llama = Llama("./mistral-7b-instruct-v0.1.Q4_K_M.gguf", n_gpu_layers=10000, n_ctx=8192, gpu_reserve_mem=512, n_batch=256)

print("---")
print("---")

# schema = r'''
# root ::= Entitylist
# Entity ::= "{"   ws   "\"name\":"   ws   string   ","   ws   "\"type\":"   ws   string   "}"
# Entitylist ::= "[]" | "["   ws   Entity   (","   ws   Entity)*   "]"
# Answer ::= "{"   ws   "\"results\":"   ws   Entitylist   "}"
# Answerlist ::= "[]" | "["   ws   Answer   (","   ws   Answer)*   "]"
# string ::= "\""   ([^"]*)   "\""
# boolean ::= "true" | "false"
# ws ::= [ \t\n]*
# number ::= [0-9]+   "."?   [0-9]*
# stringlist ::= "["   ws   "]" | "["   ws   string   (","   ws   string)*   ws   "]"
# numberlist ::= "["   ws   "]" | "["   ws   string   (","   ws   number)*   ws   "]"
#
# '''

schema = r'''
root ::= Entities
Information ::= "{"   ws   "\"type\":"   ws   string   ","   ws   "\"value\":"   ws   string   "}"
Informationlist ::= "[]" | "["   ws   Information   (","   ws   Information)*   "]"
Entity ::= "{"   ws   "\"name\":"   ws   string   ","   ws   "\"type\":"   ws   string   ","   ws   "\"informations\":"   ws   Informationlist   "}"
Entitylist ::= "[]" | "["   ws   Entity   (","   ws   Entity)*   "]"
Entities ::= "{"   ws   "\"entities\":"   ws   Entitylist   "}"
Entitieslist ::= "[]" | "["   ws   Entities   (","   ws   Entities)*   "]"
string ::= "\""   ([^"]*)   "\""
boolean ::= "true" | "false"
ws ::= [ \t\n]*
number ::= [0-9]+   "."?   [0-9]*
stringlist ::= "["   ws   "]" | "["   ws   string   (","   ws   string)*   ws   "]"
numberlist ::= "["   ws   "]" | "["   ws   string   (","   ws   number)*   ws   "]"
'''

grammar = LlamaGrammar.from_string(grammar=schema, verbose=False)

response = llama.create_completion(text, grammar=grammar, temperature=0.7, min_p=0, repeat_penalty=1, frequency_penalty=0, presence_penalty=0, max_tokens=8192, stream=True)
# response = llama.create_completion("Generate a JSON containing 5 cars associated with their fictive owner : their features, their owners.", grammar=grammar, temperature=0, max_tokens=8192, stream=True)

for item in response:
    print(item["choices"][0]["text"], end='', flush=True)


