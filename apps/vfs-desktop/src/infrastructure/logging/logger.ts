/**
 * Logger Service
 *
 * Centralized logging using Winston for important logs,
 * console.log for debug/development logs
 */
import winston from 'winston';

// Log levels
export enum LogLevel {
  Error = 'error',
  Warn = 'warn',
  Info = 'info',
  Debug = 'debug',
}

// Winston logger for persistent/important logs
const winstonLogger = winston.createLogger({
  level: process.env.NODE_ENV === 'production' ? 'info' : 'debug',
  format: winston.format.combine(
    winston.format.timestamp(),
    winston.format.errors({ stack: true }),
    winston.format.json(),
  ),
  defaultMeta: { service: 'ursly-vfs-desktop' },
  transports: [
    // Write all logs to console in development
    new winston.transports.Console({
      format: winston.format.combine(
        winston.format.colorize(),
        winston.format.simple(),
      ),
    }),
    // In production, you could add file transports here
    // new winston.transports.File({ filename: 'error.log', level: 'error' }),
    // new winston.transports.File({ filename: 'combined.log' }),
  ],
});

/**
 * Logger interface
 */
export interface ILogger {
  error(message: string, ...args: unknown[]): void;
  warn(message: string, ...args: unknown[]): void;
  info(message: string, ...args: unknown[]): void;
  debug(message: string, ...args: unknown[]): void;
}

/**
 * Logger implementation
 *
 * Uses Winston for important logs (error, warn, info)
 * Uses console.log for debug logs
 */
class Logger implements ILogger {
  error(message: string, ...args: unknown[]): void {
    winstonLogger.error(message, ...args);
  }

  warn(message: string, ...args: unknown[]): void {
    winstonLogger.warn(message, ...args);
  }

  info(message: string, ...args: unknown[]): void {
    winstonLogger.info(message, ...args);
  }

  debug(message: string, ...args: unknown[]): void {
    // Use console.log for debug logs (development only)
    if (process.env.NODE_ENV !== 'production') {
      console.log(`[DEBUG] ${message}`, ...args);
    }
  }
}

// Export singleton logger instance
export const logger = new Logger();

// Export convenience functions
export const logError = (message: string, ...args: unknown[]) =>
  logger.error(message, ...args);
export const logWarn = (message: string, ...args: unknown[]) =>
  logger.warn(message, ...args);
export const logInfo = (message: string, ...args: unknown[]) =>
  logger.info(message, ...args);
export const logDebug = (message: string, ...args: unknown[]) =>
  logger.debug(message, ...args);
