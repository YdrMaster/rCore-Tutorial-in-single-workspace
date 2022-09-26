use super::PROCESSOR;

pub fn exit_process() -> isize {
    let current = unsafe { PROCESSOR.current().unwrap() };
    if let Some(parent) = unsafe { PROCESSOR.get_task(current.parent) } {
        let pair = parent
            .children
            .iter()
            .enumerate()
            .find(|(_, &id)| id == current.pid);
        if let Some((idx, _)) = pair {
            parent.children.remove(idx);
            // log::debug!("parent remove child {}", parent.children.remove(idx));
        }
        for (_, &id) in current.children.iter().enumerate() {
            // log::warn!("parent insert child {}", id);
            parent.children.push(id);
        }
    }
    0
}
