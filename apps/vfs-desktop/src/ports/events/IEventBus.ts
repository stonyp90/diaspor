/**
 * Event Bus Port
 *
 * Interface for domain events
 */
import { DomainEventType } from '../../domain/enums/DomainEventType';

export interface DomainEvent {
  type: DomainEventType;
  payload: unknown;
  timestamp: string;
}

export interface IEventBus {
  /**
   * Emit a domain event
   */
  emit(event: DomainEvent): void;

  /**
   * Subscribe to domain events
   */
  subscribe(
    eventType: DomainEventType,
    handler: (event: DomainEvent) => void,
  ): () => void;

  /**
   * Unsubscribe from domain events
   */
  unsubscribe(
    eventType: DomainEventType,
    handler: (event: DomainEvent) => void,
  ): void;
}
