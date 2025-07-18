use chrono::NaiveDateTime;
use crate::types::trading::AssetInfo;

#[derive(Debug, Clone, Copy)]
pub enum ApiEnv {
    Real,
    Paper,
}

struct Domestic006Output1 {
    pdno: String,
    pchs_avg_pric: String,
}

struct Domestic006Output2 {
    dnca_tot_amt: String,
    nass_amt: String,
}

pub struct Domestic006Result {
    date: NaiveDateTime,
    output1: Vec<Domestic006Output1>,
    output2: Domestic006Output2,
}

impl Domestic006Result {
    pub fn new(date: NaiveDateTime, output1: Vec<(String,String)>, output2: (String,String)) -> Self {
        let output1 = output1
            .into_iter()
            .map(|(pdno, pchs_avg_pric)| Domestic006Output1 { pdno, pchs_avg_pric })
            .collect();
        let output2 = Domestic006Output2 { dnca_tot_amt: output2.0, nass_amt: output2.1 };
        Self { date, output1, output2 } 
    }

    pub fn get_pchs_avg_pric(&self, stockcode: String) -> Result<f64, Box<dyn std::error::Error>> {
        let avg = self.output1
            .iter()
            .find(|item| item.pdno == stockcode)
            .ok_or("종목을 찾을 수 없습니다")?.pchs_avg_pric.parse::<f64>()?;
        Ok(avg)
    }

    pub fn into(self) -> AssetInfo {
        AssetInfo::new(self.date, self.output2.nass_amt.parse::<f64>().unwrap())
    }
} 
