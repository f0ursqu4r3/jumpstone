export interface LoginParameters {
  identifier: string;
  secret: string;
  deviceId: string;
  deviceName?: string;
}

export interface LoginRequestBody {
  identifier: string;
  secret: string;
  device: {
    device_id: string;
    device_name?: string;
  };
}

export interface LoginResponse {
  access_token: string;
  access_expires_at: string;
  refresh_token: string;
  refresh_expires_at: string;
}

export interface ApiErrorDetail {
  field: string;
  message: string;
}

export interface ApiErrorResponse {
  error?: string;
  message?: string;
  details?: ApiErrorDetail[];
}
