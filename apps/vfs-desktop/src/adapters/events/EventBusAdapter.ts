/**
 * Event Bus Adapter
 *
 * Simple in-memory event bus implementation
 */
import { IEventBus, DomainEvent } from '../../ports/events/IEventBus';
import { DomainEventType } from '../../domain/enums/DomainEventType';

type EventHandler = (event: DomainEvent) => void;

export class EventBusAdapter implements IEventBus {
  private handlers: Map<DomainEventType, Set<EventHandler>> = new Map();

  emit(event: DomainEvent): void {
    const handlers = this.handlers.get(event.type);
    if (handlers) {
      handlers.forEach((handler) => handler(event));
    }
  }

  subscribe(eventType: DomainEventType, handler: EventHandler): () => void {
    if (!this.handlers.has(eventType)) {
      this.handlers.set(eventType, new Set());
    }
    const handlers = this.handlers.get(eventType);
    if (handlers) {
      handlers.add(handler);
    }

    // Return unsubscribe function
    return () => {
      this.unsubscribe(eventType, handler);
    };
  }

  unsubscribe(eventType: DomainEventType, handler: EventHandler): void {
    const handlers = this.handlers.get(eventType);
    if (handlers) {
      handlers.delete(handler);
      if (handlers.size === 0) {
        this.handlers.delete(eventType);
      }
    }
  }
}
