use std::vec;
use serde::{Serialize, Deserialize};
use std::error::Error;
use std::io;
use std::process;
use csv;


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ShrekQA {
    qid: u32,
    question: String,
    answers: Vec<Answer>,
    correct: u8
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Answer {
    pub ans: String,
    pub aid: u8,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ShrekQAbase {
    qid: u32,
    question: String,
    answers: String,
    correct: u8
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ShrekList{
    pub list: Vec<ShrekQA>
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SocketMessageQAList{
    pub sm_type: String,
    pub payload: Vec<ShrekQA>
    
}



pub fn extract_csv( ) -> Result<Vec<ShrekQA>, Box<dyn Error>>{

    let mut shrekQA_vec: Vec<ShrekQA> = vec![];
    let mut rdr = csv::Reader::from_path("../assets/shrek_qas.csv")?;
    for r in rdr.deserialize(){
        let mut ans_vec = vec![];
        let rec_base: ShrekQAbase = r?;
        let mut v = rec_base.answers.split('|');
        let mut counter = 1;
        while let Some(ans) = v.next() {
            let  new_ans = Answer{
                aid: counter,
                ans: ans.to_string(),
            };
            counter+=1;
            ans_vec.push(new_ans);

            // println!("{:?}", ans_vec);
        }
        shrekQA_vec.push(ShrekQA{
            qid: rec_base.qid,
            question: rec_base.question,
            answers: ans_vec,
            correct: rec_base.correct
        })
    }
    
    Ok(shrekQA_vec)

}

pub fn make_list() ->Vec<ShrekQA>  {
    let QALIST: Vec<ShrekQA> = vec![
        ShrekQA{
            qid: 1,
            question: String::from("What flower does Donkey search for?"),
            answers: vec![
                Answer{
                    ans: String::from("Yellow flower with blue spots"),
                    aid: 1
                },
                Answer{
                    ans: String::from("Blue flower with red thornes"),
                    aid: 2
                },
                Answer{
                    ans: String::from("Red flower with blue thrones"),
                    aid: 3
                },
                Answer{
                    ans: String::from("Green flower with blue thornes"),
                    aid: 4
                }
            ],
            correct: 3
        }
    ];
    QALIST
}

