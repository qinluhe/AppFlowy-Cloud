use client_api::Client;

use shared_entity::server_error::ErrorCode;

use crate::client::{
  constants::LOCALHOST_URL,
  utils::{generate_unique_email, REGISTERED_EMAIL, REGISTERED_PASSWORD},
};

#[tokio::test]
async fn sign_in_unknown_user() {
  let email = generate_unique_email();
  let password = "Hello123!";
  let mut c = Client::from(reqwest::Client::new(), LOCALHOST_URL);
  let err = c.sign_in_password(&email, password).await.unwrap_err();
  assert_eq!(err.code, ErrorCode::OAuthError);
  assert!(!err.message.is_empty());
}

#[tokio::test]
async fn sign_in_wrong_password() {
  let mut c = Client::from(reqwest::Client::new(), LOCALHOST_URL);

  let email = generate_unique_email();
  let password = "Hello123!";

  c.sign_up(&email, password).await.unwrap();

  let wrong_password = "Hllo123!";
  let err = c
    .sign_in_password(&email, wrong_password)
    .await
    .unwrap_err();
  assert_eq!(err.code, ErrorCode::OAuthError);
  assert!(!err.message.is_empty());
}

#[tokio::test]
async fn sign_in_unconfirmed_email() {
  let mut c = Client::from(reqwest::Client::new(), LOCALHOST_URL);

  let email = generate_unique_email();
  let password = "Hello123!";

  c.sign_up(&email, password).await.unwrap();

  let err = c.sign_in_password(&email, password).await.unwrap_err();
  assert_eq!(err.code, ErrorCode::OAuthError);
  assert!(!err.message.is_empty());
}

#[tokio::test]
async fn sign_in_success() {
  let mut c = Client::from(reqwest::Client::new(), LOCALHOST_URL);
  c.sign_in_password(&REGISTERED_EMAIL, &REGISTERED_PASSWORD)
    .await
    .unwrap();
  let token = c.token().unwrap();
  assert!(token.user.confirmed_at.is_some());

  // TODO: check that workspace is created for user
}