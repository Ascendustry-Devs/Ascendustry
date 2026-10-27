pub enum UiAction {
    SelectSlot(u32), // slot sélectionné
    UseItem,         // utiliser l'item du slot
    CloseInventory,  // fermer l'UI
    None,            // aucun action
}
