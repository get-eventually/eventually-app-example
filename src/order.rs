use chrono::{DateTime, Utc};

use async_trait::async_trait;

use futures::future;
use futures::future::{BoxFuture, FutureExt};

use serde::{Deserialize, Serialize};

use eventually::optional::Aggregate;
use eventually::store::Persisted;
use eventually::Projection;

#[derive(Debug, Default, Clone, Copy, Serialize)]
pub struct TotalOrdersProjection {
    created: u64,
    completed: u64,
    cancelled: u64,
}

impl Projection for TotalOrdersProjection {
    type SourceId = String;
    type Event = OrderEvent;
    type Error = std::convert::Infallible;

    fn project(
        &mut self,
        event: Persisted<Self::SourceId, Self::Event>,
    ) -> BoxFuture<Result<(), Self::Error>> {
        match event.take() {
            OrderEvent::Created { .. } => self.created += 1,
            OrderEvent::Completed { .. } => self.completed += 1,
            OrderEvent::Cancelled { .. } => self.cancelled += 1,
            _ => (),
        };

        future::ok(()).boxed()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderItem {
    pub item_sku: String,
    pub quantity: u8,
    pub price: f32,
}

trait VecExt {
    fn insert_or_merge(self, item: OrderItem) -> Self;
}

impl VecExt for Vec<OrderItem> {
    fn insert_or_merge(mut self, item: OrderItem) -> Self {
        self.iter_mut()
            .find(|it| item.item_sku == it.item_sku)
            .map(|it| it.quantity += item.quantity)
            .or_else(|| {
                self.push(item);
                Some(())
            });

        self
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "state")]
pub enum OrderState {
    Editable { updated_at: DateTime<Utc> },
    Complete { at: DateTime<Utc> },
    Cancelled { at: DateTime<Utc> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    #[allow(unused)]
    #[serde(skip_serializing)]
    id: String,
    created_at: DateTime<Utc>,
    items: Vec<OrderItem>,
    state: OrderState,
}

impl Order {
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    pub fn items(&self) -> &Vec<OrderItem> {
        &self.items
    }

    pub fn state(&self) -> OrderState {
        self.state
    }

    pub fn is_editable(&self) -> bool {
        if let OrderState::Editable { .. } = self.state {
            return true;
        }

        false
    }
}

#[derive(Debug)]
pub enum OrderCommand {
    Create,
    AddItem { item: OrderItem },
    Complete,
    Cancel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum OrderEvent {
    Created { id: String, at: DateTime<Utc> },
    ItemAdded { item: OrderItem, at: DateTime<Utc> },
    Completed { at: DateTime<Utc> },
    Cancelled { at: DateTime<Utc> },
}

impl OrderEvent {
    pub fn happened_at(&self) -> &DateTime<Utc> {
        match self {
            OrderEvent::Created { at, .. } => at,
            OrderEvent::ItemAdded { at, .. } => at,
            OrderEvent::Completed { at, .. } => at,
            OrderEvent::Cancelled { at, .. } => at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OrderError {
    #[error("order has already been created")]
    AlreadyCreated,
    #[error("order has not been created yet")]
    NotYetCreated,
    #[error("order can't be edited anymore")]
    NotEditable,
    #[error("order has already been cancelled")]
    AlreadyCompleted,
    #[error("order has already been completed")]
    AlreadyCancelled,
}

#[derive(Debug, Clone, Copy)]
pub struct OrderAggregate;

#[async_trait]
impl Aggregate for OrderAggregate {
    type Id = String;
    type State = Order;
    type Event = OrderEvent;
    type Command = OrderCommand;
    type Error = OrderError;

    fn apply_first(event: Self::Event) -> Result<Self::State, Self::Error> {
        if let OrderEvent::Created { id, at } = event {
            return Ok(Order {
                id,
                created_at: at,
                items: Vec::new(),
                state: OrderState::Editable { updated_at: at },
            });
        }

        Err(OrderError::NotYetCreated)
    }

    fn apply_next(mut state: Self::State, event: Self::Event) -> Result<Self::State, Self::Error> {
        match event {
            OrderEvent::Created { .. } => Err(OrderError::AlreadyCreated),

            OrderEvent::ItemAdded { item, at } => {
                if let OrderState::Editable { .. } = state.state {
                    state.state = OrderState::Editable { updated_at: at };
                    state.items = state.items.insert_or_merge(item);
                    return Ok(state);
                }

                Err(OrderError::NotEditable)
            }

            OrderEvent::Completed { at } => {
                if let OrderState::Complete { .. } = state.state {
                    return Err(OrderError::AlreadyCompleted);
                }

                if let OrderState::Editable { .. } = state.state {
                    state.state = OrderState::Complete { at };
                    return Ok(state);
                }

                Err(OrderError::NotEditable)
            }

            OrderEvent::Cancelled { at } => {
                if let OrderState::Cancelled { .. } = state.state {
                    return Err(OrderError::AlreadyCancelled);
                }

                if let OrderState::Editable { .. } = state.state {
                    state.state = OrderState::Cancelled { at };
                    return Ok(state);
                }

                Err(OrderError::NotEditable)
            }
        }
    }

    async fn handle_first(
        &self,
        id: &Self::Id,
        command: Self::Command,
    ) -> Result<Vec<Self::Event>, Self::Error>
    where
        Self: Sized,
    {
        if let OrderCommand::Create = command {
            return Ok(vec![OrderEvent::Created {
                id: id.clone(),
                at: Utc::now(),
            }]);
        }

        Err(OrderError::NotYetCreated)
    }

    async fn handle_next(
        &self,
        _id: &Self::Id,
        _state: &Self::State,
        command: Self::Command,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            OrderCommand::Create => Err(OrderError::AlreadyCreated),
            OrderCommand::AddItem { item } => Ok(vec![OrderEvent::ItemAdded {
                item,
                at: Utc::now(),
            }]),
            OrderCommand::Complete => Ok(vec![OrderEvent::Completed { at: Utc::now() }]),
            OrderCommand::Cancel => Ok(vec![OrderEvent::Cancelled { at: Utc::now() }]),
        }
    }
}
