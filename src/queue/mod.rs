mod mutex_linked_list;

trait MPMCQueue<T> {
    fn push(&self, value: T);
    fn pop(&self) -> Option<T>;
}
