use client_api::entity::workspace_dto::FolderView;
use client_api::entity::{AFUserProfile, AuthProvider};
use client_api::error::{AppResponseError, ErrorCode};
use collab_entity::{CollabType, EncodedCollab};
use database_entity::dto::*;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::collections::HashMap;
use tsify::Tsify;
use wasm_bindgen::JsValue;

macro_rules! from_struct_for_jsvalue {
  ($type:ty) => {
    impl From<$type> for JsValue {
      fn from(value: $type) -> Self {
        match serde_wasm_bindgen::to_value(&value) {
          Ok(js_value) => js_value,
          Err(err) => {
            tracing::error!("Failed to convert User to JsValue: {:?}", err);
            JsValue::NULL
          },
        }
      }
    }
  };
}

#[derive(Tsify, Serialize, Deserialize, Default, Debug)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct Configuration {
  pub compression_quality: u32,
  pub compression_buffer_size: usize,
}

#[derive(Tsify, Serialize, Deserialize, Default, Debug)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct ClientAPIConfig {
  pub base_url: String,
  pub ws_addr: String,
  pub gotrue_url: String,
  pub device_id: String,
  pub configuration: Option<Configuration>,
  pub client_id: String,
}

#[derive(Tsify, Serialize, Deserialize, Default, Debug)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct ClientResponse {
  pub code: ErrorCode,
  pub message: String,
}

from_struct_for_jsvalue!(ClientResponse);
impl From<AppResponseError> for ClientResponse {
  fn from(err: AppResponseError) -> Self {
    ClientResponse {
      code: err.code,
      message: err.message.to_string(),
    }
  }
}

#[derive(Tsify, Serialize, Deserialize)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct User {
  pub uid: String,
  pub uuid: String,
  pub email: Option<String>,
  pub name: Option<String>,
  pub latest_workspace_id: String,
  pub icon_url: Option<String>,
}

from_struct_for_jsvalue!(User);
impl From<AFUserProfile> for User {
  fn from(profile: AFUserProfile) -> Self {
    User {
      uid: profile.uid.to_string(),
      uuid: profile.uuid.to_string(),
      email: profile.email,
      name: profile.name,
      latest_workspace_id: profile.latest_workspace_id.to_string(),
      icon_url: None,
    }
  }
}

#[derive(Tsify, Serialize, Deserialize)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct UserWorkspace {
  pub user: User,
  pub visiting_workspace_id: String,
  pub workspaces: Vec<Workspace>,
}

from_struct_for_jsvalue!(UserWorkspace);

impl From<AFUserWorkspaceInfo> for UserWorkspace {
  fn from(info: AFUserWorkspaceInfo) -> Self {
    UserWorkspace {
      user: User::from(info.user_profile),
      visiting_workspace_id: info.visiting_workspace.workspace_id.to_string(),
      workspaces: info.workspaces.into_iter().map(Workspace::from).collect(),
    }
  }
}

#[derive(Tsify, Serialize, Deserialize)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct Workspaces {
  pub data: Vec<Workspace>,
}

from_struct_for_jsvalue!(Workspaces);

#[derive(Tsify, Serialize, Deserialize)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct WorkspaceFolder {
  pub view_id: String,
  pub icon: Option<String>,
  pub name: String,
  pub is_space: bool,
  pub is_private: bool,
  pub extra: Option<String>,
  pub children: Vec<WorkspaceFolder>,
}

from_struct_for_jsvalue!(WorkspaceFolder);

impl From<FolderView> for WorkspaceFolder {
  fn from(view: FolderView) -> Self {
    WorkspaceFolder {
      view_id: view.view_id,
      icon: view
        .icon
        .map(|icon| serde_json::to_string(&icon).unwrap_or_default()),
      name: view.name,
      is_space: view.is_space,
      is_private: view.is_private,
      extra: view
        .extra
        .map(|extra| serde_json::to_string(&extra).unwrap_or_default()),
      children: view
        .children
        .into_iter()
        .map(WorkspaceFolder::from)
        .collect(),
    }
  }
}

#[derive(Tsify, Serialize, Deserialize)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct Members {
  pub data: Vec<Member>,
}

from_struct_for_jsvalue!(Members);

#[derive(Tsify, Serialize, Deserialize)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct Member {
  pub name: String,
  pub email: String,
  pub role: String,
  pub avatar_url: Option<String>,
}

from_struct_for_jsvalue!(Member);

impl From<AFWorkspaceMember> for Member {
  fn from(profile: AFWorkspaceMember) -> Self {
    Member {
      name: profile.name,
      email: profile.email,
      role: match profile.role {
        AFRole::Member => "Member".to_string(),
        AFRole::Owner => "Owner".to_string(),
        AFRole::Guest => "Guest".to_string(),
      },
      avatar_url: profile.avatar_url,
    }
  }
}

#[derive(Tsify, Serialize, Deserialize)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct Workspace {
  pub workspace_id: String,
  pub database_storage_id: String,
  pub owner_uid: String,
  pub owner_name: String,
  pub workspace_type: i32,
  pub workspace_name: String,
  pub created_at: String,
  pub icon: String,
  pub member_count: Option<i32>,
}

from_struct_for_jsvalue!(Workspace);

impl From<AFWorkspace> for Workspace {
  fn from(workspace: AFWorkspace) -> Self {
    Workspace {
      workspace_id: workspace.workspace_id.to_string(),
      database_storage_id: workspace.database_storage_id.to_string(),
      owner_uid: workspace.owner_uid.to_string(),
      owner_name: workspace.owner_name,
      workspace_type: workspace.workspace_type,
      workspace_name: workspace.workspace_name,
      created_at: workspace.created_at.timestamp().to_string(),
      icon: workspace.icon,
      member_count: workspace.member_count.map(|count| count as i32),
    }
  }
}

#[derive(Tsify, Serialize, Deserialize, Default, Debug)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct ClientQueryCollabParams {
  pub workspace_id: String,
  pub object_id: String,
  #[tsify(type = "0 | 1 | 2 | 3 | 4 | 5")]
  pub collab_type: i32,
}

impl From<ClientQueryCollabParams> for QueryCollabParams {
  fn from(value: ClientQueryCollabParams) -> QueryCollabParams {
    QueryCollabParams {
      workspace_id: value.workspace_id,
      inner: QueryCollab {
        collab_type: CollabType::from(value.collab_type),
        object_id: value.object_id,
      },
    }
  }
}

#[derive(Tsify, Serialize, Deserialize, Default)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct ClientEncodeCollab {
  pub state_vector: Vec<u8>,
  pub doc_state: Vec<u8>,
  #[serde(default)]
  pub version: ClientEncoderVersion,
}

#[derive(Tsify, Default, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum ClientEncoderVersion {
  #[default]
  V1 = 0,
  V2 = 1,
}

from_struct_for_jsvalue!(ClientEncodeCollab);

impl From<EncodedCollab> for ClientEncodeCollab {
  fn from(collab: EncodedCollab) -> Self {
    ClientEncodeCollab {
      state_vector: collab.state_vector.to_vec(),
      doc_state: collab.doc_state.to_vec(),
      version: ClientEncoderVersion::V1,
    }
  }
}

#[derive(Tsify, Serialize, Deserialize, Default, Debug)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct BatchClientQueryCollab(pub Vec<ClientQueryCollab>);
#[derive(Tsify, Serialize, Deserialize, Default, Debug)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct ClientQueryCollab {
  pub object_id: String,
  #[tsify(type = "0 | 1 | 2 | 3 | 4 | 5")]
  pub collab_type: i32,
}

from_struct_for_jsvalue!(ClientQueryCollab);

impl From<ClientQueryCollab> for QueryCollab {
  fn from(value: ClientQueryCollab) -> QueryCollab {
    QueryCollab {
      collab_type: CollabType::from(value.collab_type),
      object_id: value.object_id,
    }
  }
}

#[derive(Tsify, Serialize, Deserialize, Default)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct BatchClientEncodeCollab(pub HashMap<String, ClientEncodeCollab>);

from_struct_for_jsvalue!(BatchClientEncodeCollab);

impl From<BatchQueryCollabResult> for BatchClientEncodeCollab {
  fn from(result: BatchQueryCollabResult) -> Self {
    let mut hash_map = HashMap::new();

    result.0.into_iter().for_each(|(k, v)| match v {
      QueryCollabResult::Success { encode_collab_v1 } => {
        EncodedCollab::decode_from_bytes(&encode_collab_v1)
          .map(|collab| {
            hash_map.insert(k, ClientEncodeCollab::from(collab));
          })
          .unwrap_or_else(|err| {
            tracing::error!("Failed to decode collab: {:?}", err);
          });
      },
      QueryCollabResult::Failed { .. } => {
        tracing::error!("Failed to get collab: {:?}", k);
      },
    });

    BatchClientEncodeCollab(hash_map)
  }
}

#[derive(Tsify, Serialize, Deserialize, Default, Debug)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct PublishViewMeta {
  pub data: String,
}

#[derive(Tsify, Serialize, Deserialize, Default, Debug)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct PublishViewPayload {
  pub meta: PublishViewMeta,
  /// The doc_state of the encoded collab.
  pub data: Vec<u8>,
}
#[derive(Tsify, Serialize, Deserialize, Default, Debug)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct PublishInfo {
  pub namespace: Option<String>,
  pub publish_name: String,
}
from_struct_for_jsvalue!(PublishViewMeta);
from_struct_for_jsvalue!(PublishViewPayload);
from_struct_for_jsvalue!(PublishInfo);

pub fn parse_provider(provider: &str) -> AuthProvider {
  match provider {
    "google" => AuthProvider::Google,
    "github" => AuthProvider::Github,
    "discord" => AuthProvider::Discord,
    _ => AuthProvider::Google,
  }
}

#[derive(Tsify, Serialize, Deserialize, Default, Debug)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct OAuthURLResponse {
  pub url: String,
}

from_struct_for_jsvalue!(OAuthURLResponse);

#[derive(Tsify, Serialize, Deserialize)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct DuplicatePublishViewPayload {
  pub workspace_id: String,
  #[tsify(type = "0 | 1 | 2 | 3 | 4 | 5 | 6")]
  pub published_collab_type: i32,
  pub published_view_id: String,
  pub dest_view_id: String,
}

from_struct_for_jsvalue!(DuplicatePublishViewPayload);

#[derive(Tsify, Serialize, Deserialize, Default, Debug)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct PublishGlobalComments {
  pub comments: Vec<PublishGlobalComment>,
}

from_struct_for_jsvalue!(PublishGlobalComments);

#[derive(Tsify, Serialize, Deserialize, Default, Debug)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct PublishGlobalComment {
  pub comment_id: String,
  pub user: Option<CommentUser>,
  pub content: String,
  pub created_at: String,
  pub last_updated_at: String,
  pub reply_comment_id: Option<String>,
  pub is_deleted: bool,
  pub can_be_deleted: bool,
}

from_struct_for_jsvalue!(PublishGlobalComment);

#[derive(Tsify, Serialize, Deserialize, Default, Debug)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct CommentUser {
  pub uuid: String,
  pub name: String,
  pub avatar_url: Option<String>,
}

from_struct_for_jsvalue!(CommentUser);

impl From<AFWebUser> for CommentUser {
  fn from(creator: AFWebUser) -> Self {
    CommentUser {
      uuid: creator.uuid.to_string(),
      name: creator.name,
      avatar_url: creator.avatar_url,
    }
  }
}

impl From<GlobalComment> for PublishGlobalComment {
  fn from(comment: GlobalComment) -> Self {
    PublishGlobalComment {
      comment_id: comment.comment_id.to_string(),
      user: comment.user.map(CommentUser::from),
      content: comment.content,
      created_at: comment.created_at.timestamp().to_string(),
      last_updated_at: comment.last_updated_at.timestamp().to_string(),
      reply_comment_id: comment.reply_comment_id.map(|id| id.to_string()),
      is_deleted: comment.is_deleted,
      can_be_deleted: comment.can_be_deleted,
    }
  }
}

#[derive(Tsify, Serialize, Deserialize, Default, Debug)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct CommentReactions {
  pub reactions: Vec<CommentReaction>,
}

from_struct_for_jsvalue!(CommentReactions);

#[derive(Tsify, Serialize, Deserialize, Default, Debug)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct CommentReaction {
  pub reaction_type: String,
  pub react_users: Vec<CommentUser>,
  pub comment_id: String,
}

from_struct_for_jsvalue!(CommentReaction);

impl From<Reaction> for CommentReaction {
  fn from(reaction: Reaction) -> Self {
    CommentReaction {
      reaction_type: reaction.reaction_type,
      react_users: reaction
        .react_users
        .into_iter()
        .map(CommentUser::from)
        .collect(),
      comment_id: reaction.comment_id.to_string(),
    }
  }
}
