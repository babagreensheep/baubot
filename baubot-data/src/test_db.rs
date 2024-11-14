use crate::*;

use baubot_utils::*;

use std::collections::HashMap;

#[derive(Default)]
pub struct TestDB {
    db: tokio::sync::Mutex<HashMap<String, i64>>,
}

impl TestDB {
    pub fn seed() -> Self {
        init();
        let user = TEST_USER.to_string();
        let chat_id = TEST_CHATID as i64;
        let mut db = HashMap::new();
        db.insert(user, chat_id);
        let db = tokio::sync::Mutex::new(db);
        Self { db }
    }
}

impl BauData for TestDB {
    fn register_user_chat_id(
        &self,
        username: &str,
    ) -> impl std::future::Future<Output = Option<i64>> + Send {
        async {
            let db = self.db.lock().await;
            let entry = db.get(username)?.to_owned();
            Some(entry)
        }
    }

    fn insert_chat_id(
        &self,
        username: &str,
        chat_id: i64,
    ) -> impl std::future::Future<Output = Result<Option<i64>, String>> + Send {
        async move {
            let mut db = self.db.lock().await;
            Ok(db.insert(username.to_string(), chat_id.clone()))
        }
    }

    fn delete_chat_id(
        &self,
        username: &str,
    ) -> impl std::future::Future<Output = Result<i64, String>> + Send {
        async move {
            let mut db = self.db.lock().await;
            match db.remove(username) {
                Some(id) => Ok(id),
                None => Err(format!(
                    "Username <code>{username}</code> was not registered."
                )),
            }
        }
    }

    fn is_admin(&self, username: &str) -> impl std::future::Future<Output = bool> + Send {
        let _ = username;
        async { true }
    }

    fn get_chat_id(&self, username: &str) -> impl std::future::Future<Output = Option<i64>> + Send {
        async move {
            let db = self.db.lock().await;
            match db.get(username) {
                Some(id) => Some(id.clone()),
                None => None,
            }
        }
    }
}
