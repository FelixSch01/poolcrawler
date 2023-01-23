use scraper::{Html, Selector};
use std::{error::Error, fmt, time::Instant, fs::File, io::Cursor};

struct Topic {
    id: String,
    title: String,
    pdf_link: String,
    exercises: Vec<Exercise>,
}

impl Topic {
    pub fn new(element: scraper::element_ref::ElementRef) -> Option<Self> {
        let id = element
            .value()
            .attr("id")
            .expect("could not find id")
            .to_string();
        
        let mut title: String = String::new();
        for text_element in element
            .select(&Selector::parse("h3").unwrap()) {
            for text in text_element.text() {
                title.push_str(text);   
            }
        }
        
        if id.is_empty() && title.is_empty() {
            None
        }
        else {
            Some(Self {
                id: id,
                title: title,
                pdf_link: String::new(),
                exercises: Vec::new(),
            })
        }
    }

    pub fn load_exercises(&mut self) {
        let response = reqwest::blocking::Client::new()
            .post("https://prod.aufgabenpool.at/srdp/phpcode/desk_results.php")
            .form(&[("id", self.id.clone())])
            .send().unwrap()
            .text().unwrap();
        let document = Html::parse_document(&response);
        let selector = Selector::parse(r"li > div > table > tbody > tr").unwrap();
        
        let mut fix: Vec<Exercise> = Vec::new();
        // Hardcoded handling of a problem with the API that
        // is called later. For some reason, for topic  B_W2_3.6
        // the order of the exercises breaks the API if a
        // certain exercise is in a particular position.
        
        for thing in document.select(&selector) {
            match Exercise::new(thing) {
                Some(x) => {
                    if x.id == "teilb1;Liftgesellschaft (2) *;a;B_435;592" {
                        fix.push(x)           
                    } else {
                        self.exercises.push(x)
                    }
                },
                None => (),
            };
        }
        
        self.exercises.append(&mut fix);
    }

    pub fn load_pdf_link(&mut self) {
        self.pdf_link.push_str("https://prod.aufgabenpool.at/srdp/createpdf.php?coll=");
        for exercise in &self.exercises {
            self.pdf_link.push(':');
            self.pdf_link.push_str(&exercise.id);
        }
    }
}

impl fmt::Display for Topic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut exercises_text = String::new();
        for exercise in &self.exercises {
            exercises_text.push_str(&format!("\n{},", exercise));
        }
        write!(formatter, "{{id=\"{}\",\ntitle=\"{}\",\nexercises={{{}}}", self.id, self.title, exercises_text)
    }
}

struct Exercise {
    id: String,
    title: String,
}

impl Exercise {
    pub fn new(element: scraper::element_ref::ElementRef) -> Option<Self> {
        let id = match element
            .select(&Selector::parse("td > input").unwrap())
            .next() {
            Some(x) => x.value()
                .attr("value")
                .expect("Could not find identifier!")
                .to_string(),
            None => String::new(),
        };

        let title = match element
            .select(&Selector::parse("td").unwrap())
            .next() {
            Some(x) => match x.text().next() {
                Some(x) => x.to_string(),
                None => String::new(),
            },
            None => String::new(),
        };
        
        if id.is_empty() && title.is_empty() {
            None
        }
        else {
            Some(Self {
                id: id,
                title: title,
            })
        }
    }
}

impl fmt::Display for Exercise {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{{id=\"{}\", title=\"{}\"}}", self.id, self.title)
    }
}

/*
 *  base = https://prod.aufgabenpool.at/srdp/phpcode
 *  on path /search_desk.php
 *      query=_ will display all results
 *      the returned html gives desired text inside a <h3>
 *      the ID for each result is stored in the <div class="results_desk"> around the <h3>
 *  on path /desk_results.php
 *      id=<ID> where <ID> is replace with the result id from /search_desk.php will
 *      return a list of all exercises in this topic
 */

struct Topics {
    topics: Vec<Topic>,
}

impl Topics {
    pub fn new() -> Self{
        Self {
            topics: Vec::new(),
        }
    }

    pub fn load_topics(&mut self) {
        let weblink = "https://prod.aufgabenpool.at/srdp/phpcode/search_desk.php";
        let response = reqwest::blocking::Client::new()
            .post(weblink)
            .form(&[("query", "_")])
            .send().unwrap()
            .text().unwrap();
        let document = Html::parse_document(&response);
        let selector = Selector::parse(r"li > div").unwrap();
        for topic in document.select(&selector) {
            match Topic::new(topic){
                Some(mut x) => {
                    x.load_exercises();
                    x.load_pdf_link();
                    self.topics.push(x)
                },
                None => (),
            };
        }
    }

    pub fn download_files(&self) {
        for topic in &self.topics {
            let response = reqwest::blocking::Client::new()
                .get(topic.pdf_link.clone())
                .send().unwrap()
                .bytes().unwrap();
           
            let limit = topic.title.chars().map(|c| c.len_utf8()).take(13).sum();
            let mut file_name = topic.title[..limit].to_string().clone();
            file_name.push_str(".pdf");

            let mut file = File::create(file_name).unwrap();
            let mut content = Cursor::new(response);

            std::io::copy(&mut content, &mut file)
                .expect("Could not create file!");
        }
    } 
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut topics = Topics::new();
    println!("Loading avialble topics and exercises...");

    let start = Instant::now();
    topics.load_topics();
    
    println!("Completed in {}s", start.elapsed().as_secs());
    println!("Downloading PDFs... (1 file per topic, containing all exercises)");

    let start = Instant::now();

    topics.download_files();
    println!("Completed in {}s", start.elapsed().as_secs());

    Ok(())
}
