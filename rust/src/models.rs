use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Phone {
    pub id: i64,
    pub label: String,
    pub number: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Email {
    pub id: i64,
    pub label: String,
    pub address: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Address {
    pub id: i64,
    pub label: String,
    pub street: String,
    pub city: String,
    pub country: String,
    pub postal: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Card {
    pub id: i64,
    pub name: String,
    pub title: String,
    pub company: String,
    pub website: String,
    pub notes: String,
    pub photo_url: String,
    pub phones: Vec<Phone>,
    pub emails: Vec<Email>,
    pub addresses: Vec<Address>,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TagCount {
    pub name: String,
    pub count: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HealthResponse {
    pub status: String,
    pub db: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct CardFormPhoneInput {
    pub label: String,
    pub number: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct CardFormEmailInput {
    pub label: String,
    pub address: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct CardFormAddressInput {
    pub label: String,
    pub street: String,
    pub city: String,
    pub country: String,
    pub postal: String,
}

#[derive(Debug, Clone, Default)]
pub struct CardInput {
    pub name: String,
    pub title: String,
    pub company: String,
    pub website: String,
    pub notes: String,
    pub phones: Vec<CardFormPhoneInput>,
    pub emails: Vec<CardFormEmailInput>,
    pub addresses: Vec<CardFormAddressInput>,
    pub tags: Vec<String>,
}
