/**
 * Placeholder OpenAPI-derived types until the backend generator lands.
 * This module is safe to import via `#shared-types` and will be replaced once
 * the shared schema pipeline is wired up.
 */

export interface SessionDeviceMetadata {
  device_id: string;
  device_name?: string | null;
}

export interface LoginRequest {
  identifier: string;
  secret: string;
  device: SessionDeviceMetadata;
}

export interface LoginResponse {
  access_token: string;
  access_expires_at: string;
  refresh_token: string;
  refresh_expires_at: string;
}

export interface RegisterRequest {
  username: string;
  password: string;
}

export interface RegisterResponse {
  user_id: string;
  username: string;
}

export interface RefreshRequest {
  refresh_token: string;
}

export type RefreshResponse = LoginResponse;

export type FieldError = {
  field: string;
  message: string;
};

export interface ValidationError {
  error: 'validation_error';
  details: FieldError[];
}

export interface InvalidCredentialsError {
  error: 'invalid_credentials';
}

export interface UsernameTakenError {
  error: 'username_taken';
}

export interface InvalidRefreshTokenError {
  error: 'invalid_refresh_token';
}

export type ApiError =
  | ValidationError
  | InvalidCredentialsError
  | UsernameTakenError
  | InvalidRefreshTokenError
  | { error: string; [key: string]: unknown };
