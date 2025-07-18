use crate::api::koreainvestapi::{execute_order, check_fill, cancel_order};
use crate::api::db_api::{execute_order_from_db, check_fill_from_db, cancel_order_from_db};
use crate::db_manager::DBManager;
use crate::types::broker::{Broker, BrokerType, Order};
use crate::types::api::ApiEnv;
use chrono::Duration;
use std::error::Error;
use std::thread;

fn execute_common(order: &Order, db: &DBManager, env: ApiEnv) -> Result<(), Box<dyn Error>> {
    let order_id = execute_order(order, env)?;
    let filled = check_fill(&order_id, env)?;
    if filled {
        db.save_trading(order.to_trading())?;
    }
    thread::sleep(Duration::minutes(5).to_std().unwrap());
    cancel_order(&order_id, env)?;
    Ok(())
}

pub struct RealBroker;
impl Broker for RealBroker {
    fn validate(&self, _order: &Order) -> Result<(), Box<dyn Error>> {
        // TODO: add real validation logic
        Ok(())
    }

    fn execute(&self, order: &Order, db: &DBManager) -> Result<(), Box<dyn Error>> {
        self.validate(order)?;
        execute_common(order, db, ApiEnv::Real)
    }
}

pub struct PaperBroker;
impl Broker for PaperBroker {
    fn validate(&self, order: &Order) -> Result<(), Box<dyn Error>> {
        RealBroker.validate(order)
    }

    fn execute(&self, order: &Order, db: &DBManager) -> Result<(), Box<dyn Error>> {
        self.validate(order)?;
        execute_common(order, db, ApiEnv::Paper)
    }
}

pub struct DbBroker;
impl Broker for DbBroker {
    fn validate(&self, _order: &Order) -> Result<(), Box<dyn Error>> { Ok(()) }

    fn execute(&self, order: &Order, db: &DBManager) -> Result<(), Box<dyn Error>> {
        self.validate(order)?;
        let order_id = execute_order_from_db(order)?;
        let filled = check_fill_from_db(&order_id)?;
        if filled {
            db.save_trading(order.to_trading())?;
        }
        thread::sleep(Duration::minutes(5).to_std().unwrap());
        cancel_order_from_db(&order_id)?;
        Ok(())
    }
}

pub fn make_broker(kind: BrokerType) -> Box<dyn Broker> {
    match kind {
        BrokerType::REAL => Box::new(RealBroker),
        BrokerType::PAPER => Box::new(PaperBroker),
        BrokerType::DB => Box::new(DbBroker),
    }
}
