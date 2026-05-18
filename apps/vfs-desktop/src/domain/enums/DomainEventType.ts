/**
 * Domain Event Type Enum
 *
 * Types of domain events that can be emitted
 */
export enum DomainEventType {
  FileCreated = 'file.created',
  FileDeleted = 'file.deleted',
  FileRenamed = 'file.renamed',
  FileMoved = 'file.moved',
  StorageConnected = 'storage.connected',
  StorageDisconnected = 'storage.disconnected',
  OperationStarted = 'operation.started',
  OperationCompleted = 'operation.completed',
  OperationFailed = 'operation.failed',
}
