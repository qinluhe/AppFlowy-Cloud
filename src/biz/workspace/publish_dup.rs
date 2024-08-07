use app_error::AppError;
use appflowy_collaborate::collab::storage::CollabAccessControlStorage;
use collab::core::collab::DataSource;
use collab::preclude::Collab;
use collab_database::database::Database;
use collab_database::views::ViewMap;
use collab_database::workspace_database::{DatabaseMetaList, WorkspaceDatabase};
use collab_document::document::Document;
use collab_entity::CollabType;
use collab_folder::{
  CollabOrigin, Folder, RepeatedViewIdentifier, View, ViewIcon, ViewIdentifier, ViewLayout,
};
use collab_rt_entity::user::RealtimeUser;
use collab_rt_entity::{ClientCollabMessage, UpdateSync};
use collab_rt_protocol::{Message, SyncMessage};
use database::collab::{select_workspace_database_oid, CollabStorage};
use database::publish::select_published_data_for_view_id;
use database_entity::dto::CollabParams;
use sqlx::PgPool;
use std::{collections::HashMap, sync::Arc};
use yrs::updates::encoder::Encode;

use crate::biz::collab::ops::get_latest_collab_encoded;
use crate::state::AppStateGroupManager;

#[allow(clippy::too_many_arguments)]
pub async fn duplicate_published_collab_to_workspace(
  pg_pool: &PgPool,
  collab_storage: Arc<CollabAccessControlStorage>,
  group_manager: AppStateGroupManager,
  dest_uid: i64,
  publish_view_id: String,
  dest_workspace_id: String,
  dest_view_id: String,
  collab_type: CollabType,
) -> Result<(), AppError> {
  let copier = PublishCollabDuplicator::new(
    pg_pool.clone(),
    collab_storage.clone(),
    group_manager,
    dest_uid,
    dest_workspace_id,
    dest_view_id,
  );
  copier.deep_copy(&publish_view_id, collab_type).await?;
  Ok(())
}

pub struct PublishCollabDuplicator {
  /// for fetching and writing folder data
  /// of dest workspace
  collab_storage: Arc<CollabAccessControlStorage>,
  /// A map to store the old view_id that was duplicated and new view_id assigned.
  /// If value is none, it means the view_id is not published.
  duplicated_refs: HashMap<String, Option<String>>,
  /// in case there's existing group, which contains the most updated collab data
  group_manager: AppStateGroupManager,
  /// A list of new views to be added to the folder
  views_to_add: Vec<View>,
  /// A list of database linked views to be added to workspace database
  workspace_databases: HashMap<String, Vec<String>>,
  /// time of duplication
  ts_now: i64,
  /// for fetching published data
  /// and writing them to dest workspace
  pg_pool: PgPool,
  /// user initiating the duplication
  duplicator_uid: i64,
  /// workspace to duplicate into
  dest_workspace_id: String,
  /// view of workspace to duplicate into
  dest_view_id: String,
}

impl PublishCollabDuplicator {
  pub fn new(
    pg_pool: PgPool,
    collab_storage: Arc<CollabAccessControlStorage>,
    group_manager: AppStateGroupManager,
    dest_uid: i64,
    dest_workspace_id: String,
    dest_view_id: String,
  ) -> Self {
    let ts_now = chrono::Utc::now().timestamp();
    Self {
      ts_now,
      duplicated_refs: HashMap::new(),
      views_to_add: Vec::new(),
      workspace_databases: HashMap::new(),

      pg_pool,
      collab_storage,
      group_manager,
      duplicator_uid: dest_uid,
      dest_workspace_id,
      dest_view_id,
    }
  }

  async fn deep_copy(
    mut self,
    publish_view_id: &str,
    collab_type: CollabType,
  ) -> Result<(), AppError> {
    let mut txn = self.pg_pool.begin().await?;

    // new view after deep copy
    // this is the root of the document/database duplicated
    let mut root_view = match self
      .deep_copy_txn(
        &mut txn,
        uuid::Uuid::new_v4().to_string(),
        publish_view_id,
        collab_type.clone(),
      )
      .await?
    {
      Some(v) => v,
      None => {
        txn.rollback().await?;
        return Err(AppError::RecordNotFound(
          "view not found, it might be unpublished".to_string(),
        ));
      },
    };
    root_view.parent_view_id = self.dest_view_id.clone();

    let collab_folder_encoded = get_latest_collab_encoded(
      self.group_manager.clone(),
      self.collab_storage.clone(),
      &self.duplicator_uid,
      &self.dest_workspace_id,
      &self.dest_workspace_id,
      CollabType::Folder,
    )
    .await?;

    let folder = Folder::from_collab_doc_state(
      self.duplicator_uid,
      CollabOrigin::Server,
      DataSource::DocStateV1(collab_folder_encoded.doc_state.to_vec()),
      &self.dest_workspace_id,
      vec![],
    )
    .map_err(|e| AppError::Unhandled(e.to_string()))?;

    let encoded_update = folder.get_updates_for_op(|folder| {
      // add all views required to the folder
      folder.insert_view(root_view, None);
      for view in &self.views_to_add {
        folder.insert_view(view.clone(), None);
      }
    });

    // update folder collab
    let updated_encoded_collab = folder
      .encode_collab_v1()
      .map_err(|e| AppError::Unhandled(e.to_string()))?;

    // insert updated folder collab
    self
      .insert_collab_for_duplicator(
        &self.dest_workspace_id.clone(),
        updated_encoded_collab.encode_to_bytes()?,
        CollabType::Folder,
      )
      .await?;

    // broadcast folder changes
    self
      .broadcast_update(&self.dest_workspace_id, encoded_update)
      .await;

    // update database if any
    if !self.workspace_databases.is_empty() {
      let ws_db_oid = select_workspace_database_oid(&self.pg_pool, &self.dest_workspace_id).await?;
      let ws_db_collab = {
        let ws_database_ec = get_latest_collab_encoded(
          self.group_manager.clone(),
          self.collab_storage.clone(),
          &self.duplicator_uid,
          &self.dest_workspace_id,
          &ws_db_oid,
          CollabType::WorkspaceDatabase,
        )
        .await?;
        Collab::new_with_source(
          CollabOrigin::Server,
          &ws_db_oid,
          DataSource::DocStateV1(ws_database_ec.doc_state.to_vec()),
          vec![],
          false,
        )
        .map_err(|e| AppError::Unhandled(e.to_string()))?
      };

      let ws_db_meta_list = DatabaseMetaList::from_collab(&ws_db_collab);
      let ws_db_updates = {
        let mut txn_wrapper = ws_db_collab.origin_transact_mut();
        for (db_collab_id, linked_views) in &self.workspace_databases {
          ws_db_meta_list.add_database_with_txn(
            &mut txn_wrapper,
            db_collab_id,
            linked_views.clone(),
          );
        }
        txn_wrapper.encode_update_v1()
      };
      self.broadcast_update(&ws_db_oid, ws_db_updates).await;
      let updated_ws_w_db_collab = ws_db_collab
        .encode_collab_v1(WorkspaceDatabase::validate)
        .map_err(|e| AppError::Unhandled(e.to_string()))?;
      self
        .insert_collab_for_duplicator(
          &ws_db_collab.object_id,
          updated_ws_w_db_collab.encode_to_bytes()?,
          CollabType::WorkspaceDatabase,
        )
        .await?;
    }

    txn.commit().await?;
    Ok(())
  }

  /// Deep copy a published collab to the destination workspace.
  /// If None is returned, it means the view is not published.
  /// If Some is returned, a new view is created but without parent_view_id set.
  /// Caller should set the parent_view_id to the parent view.
  async fn deep_copy_txn(
    &mut self,
    txn: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    new_view_id: String,
    publish_view_id: &str,
    collab_type: CollabType,
  ) -> Result<Option<View>, AppError> {
    // attempt to get metadata and doc_state for published view
    let (metadata, published_blob) =
      match select_published_data_for_view_id(txn, &publish_view_id.parse()?).await? {
        Some(published_data) => published_data,
        None => {
          tracing::warn!(
            "No published collab data found for view_id: {}",
            publish_view_id
          );
          return Ok(None);
        },
      };

    // at this stage, we know that the view is published,
    // so we insert this knowledge into the duplicated_refs
    self
      .duplicated_refs
      .insert(publish_view_id.to_string(), new_view_id.clone().into());

    match collab_type {
      CollabType::Document => {
        let doc = Document::from_doc_state(
          CollabOrigin::Empty,
          DataSource::DocStateV1(published_blob.to_vec()),
          "",
          vec![],
        )
        .map_err(|e| AppError::Unhandled(e.to_string()))?;

        let new_doc_view = self
          .deep_copy_doc_txn(txn, new_view_id, doc, metadata)
          .await?;
        Ok(Some(new_doc_view))
      },
      CollabType::Database => {
        let db_json_obj = serde_json::from_slice::<serde_json::Value>(&published_blob).unwrap();
        let new_db_view = self
          .deep_copy_database_txn(txn, new_view_id, db_json_obj, metadata)
          .await?;
        Ok(Some(new_db_view))
      },
      CollabType::DatabaseRow => {
        // TODO
        Ok(None)
      },
      t => {
        tracing::warn!("collab type not supported: {:?}", t);
        Ok(None)
      },
    }
  }

  async fn deep_copy_doc_txn<'a>(
    &mut self,
    txn: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    new_view_id: String,
    doc: Document,
    metadata: serde_json::Value,
  ) -> Result<View, AppError> {
    let mut ret_view = self.new_view(new_view_id.clone(), &metadata);

    let mut doc_data = doc
      .get_document_data()
      .map_err(|e| AppError::Unhandled(e.to_string()))?;

    let page_ids = doc_data
      .blocks
      .values_mut()
      .flat_map(|block| block.data.iter_mut())
      .filter(|(key, _)| *key == "delta")
      .flat_map(|(_, value)| value.as_array_mut())
      .flatten()
      .flat_map(|delta| delta.get_mut("attributes"))
      .flat_map(|attributes| attributes.get_mut("mention"))
      .filter(|mention| {
        mention.get("type").map_or(false, |type_| {
          type_.as_str().map_or(false, |type_| type_ == "page")
        })
      })
      .flat_map(|mention| mention.get_mut("page_id"));

    // deep copy all the page_id references
    for page_id in page_ids {
      let page_id_str = match page_id.as_str() {
        Some(page_id_str) => page_id_str,
        None => continue,
      };
      match self.duplicated_refs.get_key_value(page_id_str) {
        Some((_old_view_id, new_view_id)) => {
          if let Some(vid) = new_view_id {
            *page_id = serde_json::json!(vid);
            ret_view
              .children
              .items
              .push(ViewIdentifier { id: vid.clone() });
          } else {
            // ref view_id is not published
            // TODO: handle this case to
            // display better in the UI?
          }
        },
        None => {
          // Call deep_copy_txn and await the result
          if let Some(mut new_view) = Box::pin(self.deep_copy_txn(
            txn,
            uuid::Uuid::new_v4().to_string(),
            page_id_str,
            CollabType::Document,
          ))
          .await?
          {
            new_view.parent_view_id = ret_view.id.clone();
            ret_view.children.items.push(ViewIdentifier {
              id: new_view.id.clone(),
            });
            self
              .duplicated_refs
              .insert(page_id_str.to_string(), Some(new_view.id.clone()));
            self.views_to_add.push(new_view.clone());
            *page_id = serde_json::json!(new_view.id);
          } else {
            self.duplicated_refs.insert(page_id_str.to_string(), None);
          }
        },
      }
    }

    // update text map
    if let Some(text_map) = doc_data.meta.text_map.as_mut() {
      for (_key, value) in text_map.iter_mut() {
        let mut js_val = match serde_json::from_str::<serde_json::Value>(value) {
          Ok(js_val) => js_val,
          Err(e) => {
            tracing::error!("failed to parse text_map value({}): {}", value, e);
            continue;
          },
        };
        let js_array = match js_val.as_array_mut() {
          Some(js_array) => js_array,
          None => continue,
        };
        js_array
          .iter_mut()
          .flat_map(|js_val| js_val.get_mut("attributes"))
          .flat_map(|attributes| attributes.get_mut("mention"))
          .filter(|mention| {
            mention.get("type").map_or(false, |type_| {
              type_.as_str().map_or(false, |type_| type_ == "page")
            })
          })
          .flat_map(|mention| mention.get_mut("page_id"))
          .for_each(|page_id| {
            let page_id_str = match page_id.as_str() {
              Some(page_id_str) => page_id_str,
              None => return,
            };
            if let Some(new_page_id) = self.duplicated_refs.get(page_id_str) {
              *page_id = serde_json::json!(new_page_id);
            }
          });
        *value = js_val.to_string();
      }
    }

    // doc_data into binary data
    let new_doc_data = {
      let collab = doc.get_collab().clone();
      let new_doc = Document::create_with_data(collab, doc_data)
        .map_err(|e| AppError::Unhandled(e.to_string()))?;
      let encoded_collab = new_doc
        .encode_collab()
        .map_err(|e| AppError::Unhandled(e.to_string()))?;
      encoded_collab.encode_to_bytes()?
    };

    // insert document with modified page_id references
    self
      .insert_collab_for_duplicator(&ret_view.id, new_doc_data, CollabType::Document)
      .await?;

    Ok(ret_view)
  }

  fn new_view(&self, new_view_id: String, metadata: &serde_json::Value) -> View {
    let (name, icon, extra) = match metadata.get("view") {
      Some(view) => {
        let name = view
          .get("name")
          .and_then(|name| name.as_str())
          .unwrap_or("Untitled Duplicated");
        let icon = view
          .get("icon")
          .and_then(|icon| serde_json::from_value::<ViewIcon>(icon.clone()).ok());
        let extra = view.get("extra").and_then(|name| name.as_str());
        (name, icon, extra)
      },
      None => ("Untitled Duplicated", None, None),
    };

    View {
      id: new_view_id,
      parent_view_id: "".to_string(), // to be filled by caller
      name: name.to_string(),
      desc: "".to_string(), // unable to get from metadata
      children: RepeatedViewIdentifier { items: vec![] }, // fill in while iterating children
      created_at: self.ts_now,
      is_favorite: false,
      layout: ViewLayout::Document,
      icon,
      created_by: Some(self.duplicator_uid),
      last_edited_time: self.ts_now,
      last_edited_by: Some(self.duplicator_uid),
      extra: extra.map(String::from),
    }
  }

  async fn deep_copy_database_txn<'a>(
    &mut self,
    _txn: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    new_view_id: String,
    published_db: serde_json::Value,
    metadata: serde_json::Value,
  ) -> Result<View, AppError> {
    // create a new view to be returned to the caller
    let ret_view = self.new_view(new_view_id.clone(), &metadata);

    let db_collab = {
      let db_bin_data = published_db
        .get("database_collab")
        .ok_or_else(|| AppError::RecordNotFound("database_collab not found".to_string()))?
        .as_array()
        .ok_or_else(|| AppError::RecordNotFound("database_collab not an array".to_string()))?
        .iter()
        .map(|v| v.as_number().unwrap().as_u64().unwrap())
        .map(|v| v as u8)
        .collect::<Vec<_>>();
      Collab::new_with_source(
        CollabOrigin::Server,
        &new_view_id,
        DataSource::DocStateV1(db_bin_data),
        vec![],
        false,
      )
      .map_err(|e| AppError::Unhandled(e.to_string()))?
    };

    let published_rows = published_db
      .get("database_row_collabs")
      .ok_or_else(|| AppError::RecordNotFound("database_row_collabs not found".to_string()))?;

    let mut txn = db_collab.origin_transact_mut();

    // create new identity for database
    let new_db_uuid = uuid::Uuid::new_v4().to_string();
    if let Some(container) = db_collab.get_map_with_txn(txn.txn(), vec!["database", "fields"]) {
      container.insert_with_txn(&mut txn, "id", new_db_uuid.clone());
    }

    // Add this database as linked view
    self
      .workspace_databases
      .insert(new_db_uuid.clone(), vec![new_view_id]);

    // Set the row_id references
    if let Some(container) = db_collab.get_map_with_txn(txn.txn(), vec!["database", "views"]) {
      let view_change_tx = tokio::sync::broadcast::channel(1).0;
      let views = ViewMap::new(container, view_change_tx);
      let mut reset_views = views.get_all_views_with_txn(txn.txn());
      for db_view in reset_views.iter_mut() {
        for row_order in db_view.row_orders.iter_mut() {
          let row = published_rows.get(row_order.id.as_str()).unwrap();
          let bin_data = row
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_number().unwrap().as_u64().unwrap())
            .map(|v| v as u8)
            .collect::<Vec<_>>();

          let new_row_uuid = uuid::Uuid::new_v4().to_string();
          let db_row_collab = Collab::new_with_source(
            CollabOrigin::Server,
            new_row_uuid.as_str(),
            DataSource::DocStateV1(bin_data),
            vec![],
            false,
          )
          .unwrap();

          db_row_collab.with_origin_transact_mut(|txn| {
            if let Some(container) = db_row_collab.get_map_with_txn(txn, vec!["data"]) {
              // TODO(Zack): deep copy row data ?
              container.insert_with_txn(txn, "id", new_row_uuid.clone());
              container.insert_with_txn(txn, "database_id", new_db_uuid.clone());
            }
          });

          match db_row_collab.encode_collab_v1(Database::validate) {
            Ok(ec) => match ec.encode_to_bytes() {
              Ok(db_row_ec_bytes) => {
                self
                  .insert_collab_for_duplicator(
                    &new_row_uuid,
                    db_row_ec_bytes,
                    CollabType::DatabaseRow,
                  )
                  .await?;
              },
              Err(e) => tracing::error!("failed to encode db_row_collab: {}", e),
            },
            Err(e) => tracing::error!("failed to encode db_row_collab: {}", e),
          }
        }
      }

      for view in reset_views {
        views.insert_view_with_txn(&mut txn, view);
      }
    }

    let db_encoded_collab = db_collab
      .encode_collab_v1(Database::validate)
      .map_err(|e| AppError::Unhandled(e.to_string()))?
      .encode_to_bytes()?;

    // insert database with modified row_id references
    self
      .insert_collab_for_duplicator(&ret_view.id, db_encoded_collab, CollabType::Database)
      .await?;

    Ok(ret_view)
  }

  async fn insert_collab_for_duplicator(
    &self,
    oid: &str,
    encoded_collab: Vec<u8>,
    collab_type: CollabType,
  ) -> Result<(), AppError> {
    self
      .collab_storage
      .insert_or_update_collab(
        &self.dest_workspace_id,
        &self.duplicator_uid,
        CollabParams {
          object_id: oid.to_string(),
          encoded_collab_v1: encoded_collab,
          collab_type,
          embeddings: None,
        },
        true,
      )
      .await?;
    Ok(())
  }

  /// broadcast updates to collab group if exists
  async fn broadcast_update(&self, oid: &str, encoded_update: Vec<u8>) {
    match self.group_manager.get_group(oid).await {
      Some(group) => {
        let (collab_message_sender, _collab_message_receiver) = futures::channel::mpsc::channel(1);
        let (mut message_by_oid_sender, message_by_oid_receiver) =
          futures::channel::mpsc::channel(1);
        group
          .subscribe(
            &RealtimeUser {
              uid: self.duplicator_uid,
              device_id: uuid::Uuid::new_v4().to_string(),
              connect_at: self.ts_now,
              session_id: uuid::Uuid::new_v4().to_string(),
              app_version: "".to_string(),
            },
            CollabOrigin::Server,
            collab_message_sender,
            message_by_oid_receiver,
          )
          .await;
        let payload = Message::Sync(SyncMessage::Update(encoded_update)).encode_v1();
        let message = HashMap::from([(
          oid.to_string(),
          vec![ClientCollabMessage::ClientUpdateSync {
            data: UpdateSync {
              origin: CollabOrigin::Server,
              object_id: oid.to_string(),
              msg_id: self.ts_now as u64,
              payload: payload.into(),
            },
          }],
        )]);
        match message_by_oid_sender.try_send(message) {
          Ok(()) => tracing::info!("sent message to group"),
          Err(err) => tracing::error!("failed to send message to group: {}", err),
        }
      },
      None => tracing::warn!("group not found for oid: {}", oid),
    }
  }
}
