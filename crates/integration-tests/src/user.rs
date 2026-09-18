use bb_core::{
    repository::{read_only_transaction, transaction},
    shelf::{NewShelf, ShelfType},
};

use crate::{fixtures, setup};

/// Deleting a user whose personal library contains a shelf:
///  - removes the library
///  - deletes the shelf (not re-parented, because it belongs to the deleted
///    user)
#[tokio::test]
async fn delete_user_removes_personal_library_and_owned_shelves() {
    let ctx = setup().await;
    let user = fixtures::insert_user(&ctx.repos, "user_with_library").await;

    // Create the user's personal library.
    let lib = fixtures::insert_library(&ctx.services, user.id, "Alice's Library").await;

    // Insert a shelf directly into the personal library (bypassing
    // create_manual_shelf which always places shelves in All Books).
    let shelf_repo = ctx.repos.shelf_repository().clone();
    let shelf = transaction(&**ctx.repos.repository(), |tx| {
        let shelf_repo = shelf_repo.clone();
        Box::pin(async move {
            shelf_repo
                .add_shelf(
                    tx,
                    NewShelf {
                        owner_id: user.id,
                        library_id: lib.id,
                        name: "My Shelf".to_string(),
                        shelf_type: ShelfType::Manual,
                        device_id: None,
                        filter_criteria: None,
                    },
                )
                .await
        })
    })
    .await
    .unwrap();

    // Delete the user — service layer must delete shelves and the library
    // before removing the user row.
    ctx.services.user_service.delete_user(user.id).await.unwrap();

    // The personal library should be gone.
    let found_lib = ctx.services.library_service.find_library_by_token(lib.token).await.unwrap();
    assert!(found_lib.is_none(), "personal library should be deleted with the user");

    // The shelf should also be gone — it belongs to the deleted user.
    let shelf_repo = ctx.repos.shelf_repository().clone();
    let found_shelf = read_only_transaction(&**ctx.repos.repository(), |tx| {
        let shelf_repo = shelf_repo.clone();
        Box::pin(async move { shelf_repo.find_by_token(tx, shelf.token).await })
    })
    .await
    .unwrap();

    assert!(found_shelf.is_none(), "shelf should be deleted with the user");
}
