use serde::Serialize;
use std::time::SystemTime;

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug))]
pub struct Transaction {
    pub valor: i16,
    pub tipo: String,
    pub descricao: String,
    pub realizada_em: SystemTime,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Debug, Clone))]
pub struct Balance {
    pub total: i32,
    pub data_extrato: SystemTime,
    pub limite: i32,
}

#[cfg(test)]
mod test {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_serialization_transaction() {
        let transaction = serde_json::to_value(Transaction {
            valor: 10,
            tipo: "c".into(),
            descricao: "grocery".into(),
            realizada_em: SystemTime::UNIX_EPOCH,
        })
        .unwrap();

        assert_eq!(
            transaction,
            json!({
                "valor": 10, "tipo": "c", "descricao": "grocery", "realizada_em": SystemTime::UNIX_EPOCH,
            })
        );
    }

    #[test]
    fn test_serialization_balance() {
        let balance = serde_json::to_value(Balance {
            total: 20,
            limite: 1000,
            data_extrato: SystemTime::UNIX_EPOCH,
        })
        .unwrap();

        assert_eq!(
            balance,
            json!({
                "total": 20, "limite": 1000, "data_extrato": SystemTime::UNIX_EPOCH,
            })
        );
    }
}
