Type of entities Supported :
- Persons
- Domains
- Companies / Organisations

To find information about an entity, we can use : 
## google search engine
- Advantages : 
  - Can be very precise and reliable with dorking
- Disadvantages : 
  - Google search engine API is subject to a fee
  - Scraping is hard as the website's front end often changes is complex/obfuscated

## Google's About search page
example : https://www.google.com/search?q=About+pluralsight.com&tbm=ilp

- Advantages : 
  - Simpler frontend than google search engine results page, easier to scrap
  - Have most interesting information such as summary, social networks, important informations such as CEO / headquarters etc (these stuff can be found in emails)
- Disadvantages : 
  - Seems to be unreliable sometimes, ex : [About pluralsight](https://www.google.com/search?q=About+pluralsight&tbm=ilp), [About pluralsight.com](https://www.google.com/search?q=About+pluralsight.com&tbm=ilp) and [About pluralsight LLC](https://www.google.com/search?q=About+pluralsight+LLC&tbm=ilp) would sometime return 404

Can be used to provide some external human-readable information interface

## Wikidata 

- Advantages : 
  - Has a REST API to request data about anything
  - Returned data seems to be complete: [Pluralsight](https://www.wikidata.org/wiki/Q19757566)
  - For each data, a list of references is linked (good way to pivot)
  - Can also have "Also known as" aliases (good way to pivot)

- Disadvantages :
  - I'm scared that the provided data could be unreliable : [see Verifiability](https://www.wikidata.org/wiki/Wikidata:Verifiability#:~:text=In%20practice%2C%20this%20means%20that,data%20provided%20in%20a%20statement.)
  - A few part of provided properties seem to come with a reference, ex : [911 attacks](https://www.wikidata.org/wiki/Q10806), you can see that most of the properties have no bound reference
  - API IS A TOTAL MESS !