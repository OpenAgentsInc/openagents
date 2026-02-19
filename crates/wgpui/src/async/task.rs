use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::channel::oneshot;

#[must_use]
pub struct Task<T>(TaskState<T>);

enum TaskState<T> {
    Ready(Option<T>),
    Pending(oneshot::Receiver<T>),
}

impl<T> Task<T> {
    pub fn ready(value: T) -> Self {
        Task(TaskState::Ready(Some(value)))
    }

    pub(crate) fn from_receiver(receiver: oneshot::Receiver<T>) -> Self {
        Task(TaskState::Pending(receiver))
    }

    pub fn detach(self) {
        drop(self);
    }

    pub fn try_take(&mut self) -> Option<T> {
        match &mut self.0 {
            TaskState::Ready(value) => value.take(),
            TaskState::Pending(receiver) => match receiver.try_recv() {
                Ok(Some(value)) => {
                    self.0 = TaskState::Ready(Some(value));
                    if let TaskState::Ready(value) = &mut self.0 {
                        value.take()
                    } else {
                        None
                    }
                }
                Ok(None) => None,
                Err(_) => None,
            },
        }
    }
}

impl<T> Unpin for Task<T> {}

impl<T> Future for Task<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        match &mut this.0 {
            TaskState::Ready(value) => Poll::Ready(value.take().expect("task already completed")),
            TaskState::Pending(receiver) => match Pin::new(receiver).poll(cx) {
                Poll::Ready(Ok(value)) => {
                    this.0 = TaskState::Ready(Some(value));
                    if let TaskState::Ready(value) = &mut this.0 {
                        Poll::Ready(value.take().expect("task already completed"))
                    } else {
                        unreachable!("task state should be ready")
                    }
                }
                Poll::Ready(Err(_)) => panic!("task cancelled before completion"),
                Poll::Pending => Poll::Pending,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Task;

    #[test]
    fn task_ready_returns_value() {
        let mut task = Task::ready(42);
        assert_eq!(task.try_take(), Some(42));
        assert_eq!(task.try_take(), None);
    }
}
