import { defineStore } from 'pinia';
import { extractErrorMessage } from '~/utils/errors';
import type {
  ApiErrorResponse,
  LoginParameters,
  LoginRequestBody,
  LoginResponse,
} from '~/types/session';

const STORAGE_KEY = 'openguild.session.v1';

interface SessionTokens {
  accessToken: string;
  accessExpiresAt: string;
  refreshToken: string;
  refreshExpiresAt: string;
}

interface PersistedSession {
  identifier: string;
  deviceId: string;
  deviceName: string;
  tokens: SessionTokens | null;
}

interface SessionState extends PersistedSession {
  loading: boolean;
  error: string | null;
  fieldErrors: Record<string, string>;
}

const isIsoFuture = (iso: string | null | undefined): boolean => {
  if (!iso) {
    return false;
  }
  const parsed = Date.parse(iso);
  if (Number.isNaN(parsed)) {
    return false;
  }
  return parsed > Date.now();
};

const sanitizeTokens = (value: unknown): SessionTokens | null => {
  if (!value || typeof value !== 'object') {
    return null;
  }
  const tokens = value as Partial<Record<keyof SessionTokens, unknown>>;
  const accessToken =
    typeof tokens.accessToken === 'string' ? tokens.accessToken : '';
  const refreshToken =
    typeof tokens.refreshToken === 'string' ? tokens.refreshToken : '';
  const accessExpiresAt =
    typeof tokens.accessExpiresAt === 'string'
      ? tokens.accessExpiresAt
      : '';
  const refreshExpiresAt =
    typeof tokens.refreshExpiresAt === 'string'
      ? tokens.refreshExpiresAt
      : '';

  if (!accessToken || !refreshToken) {
    return null;
  }

  return {
    accessToken,
    refreshToken,
    accessExpiresAt,
    refreshExpiresAt,
  };
};

const readFromStorage = (): PersistedSession | null => {
  if (typeof window === 'undefined') {
    return null;
  }

  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) {
      return null;
    }

    const parsed = JSON.parse(raw) as Partial<PersistedSession>;
    return {
      identifier:
        typeof parsed.identifier === 'string' ? parsed.identifier : '',
      deviceId: typeof parsed.deviceId === 'string' ? parsed.deviceId : '',
      deviceName:
        typeof parsed.deviceName === 'string' ? parsed.deviceName : '',
      tokens: sanitizeTokens(parsed.tokens),
    };
  } catch {
    return null;
  }
};

const writeToStorage = (value: PersistedSession) => {
  if (typeof window === 'undefined') {
    return;
  }

  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(value));
  } catch {
    // ignore storage failures to avoid breaking login flows when storage is unavailable
  }
};

const initialState = (): SessionState => {
  if (import.meta.server) {
    return {
      identifier: '',
      deviceId: '',
      deviceName: '',
      tokens: null,
      loading: false,
      error: null,
      fieldErrors: {},
    };
  }

  const persisted = readFromStorage();
  const tokens =
    persisted?.tokens && isIsoFuture(persisted.tokens.accessExpiresAt)
      ? persisted.tokens
      : null;

  return {
    identifier: persisted?.identifier ?? '',
    deviceId: persisted?.deviceId ?? '',
    deviceName: persisted?.deviceName ?? '',
    tokens,
    loading: false,
    error: null,
    fieldErrors: {},
  };
};

const mapValidationField = (field: string): string => {
  if (field === 'device.device_id') {
    return 'deviceId';
  }
  if (field === 'device.device_name') {
    return 'deviceName';
  }
  return field;
};

const formatLoginError = (
  err: unknown
): { message: string; fieldErrors: Record<string, string> } => {
  const fieldErrors: Record<string, string> = {};
  const fallbackMessage =
    'Unable to sign in right now. Please try again in a moment.';

  const maybeFetchError = err as {
    data?: ApiErrorResponse;
    response?: { status?: number };
  };

  const { data } = maybeFetchError;

  if (data?.details) {
    data.details.forEach((detail) => {
      if (!detail || typeof detail.field !== 'string') {
        return;
      }
      const key = mapValidationField(detail.field);
      fieldErrors[key] = detail.message ?? 'Invalid value';
    });
  }

  if (data?.error === 'invalid_credentials') {
    return {
      message:
        'Invalid credentials. Check your identifier and secret, then try again.',
      fieldErrors,
    };
  }

  if (data?.error === 'validation_error') {
    return {
      message:
        Object.values(fieldErrors)[0] ??
        'Please fix the highlighted fields and try again.',
      fieldErrors,
    };
  }

  if (data?.message) {
    return {
      message: data.message,
      fieldErrors,
    };
  }

  if (maybeFetchError.response?.status === 401) {
    return {
      message:
        'Invalid credentials. Check your identifier and secret, then try again.',
      fieldErrors,
    };
  }

  return {
    message: extractErrorMessage(err) || fallbackMessage,
    fieldErrors,
  };
};

const toPersistedSession = (state: PersistedSession): PersistedSession => ({
  identifier: state.identifier,
  deviceId: state.deviceId,
  deviceName: state.deviceName,
  tokens: state.tokens,
});

export const useSessionStore = defineStore('session', {
  state: (): SessionState => initialState(),
  getters: {
    isAuthenticated: (state): boolean => {
      if (!state.tokens) {
        return false;
      }
      if (!state.tokens.accessToken) {
        return false;
      }
      return isIsoFuture(state.tokens.accessExpiresAt);
    },
    accessToken: (state): string => state.tokens?.accessToken ?? '',
  },
  actions: {
    resetErrors() {
      this.error = null;
      this.fieldErrors = {};
    },

    persist() {
      if (import.meta.server) {
        return;
      }
      writeToStorage(
        toPersistedSession({
          identifier: this.identifier,
          deviceId: this.deviceId,
          deviceName: this.deviceName,
          tokens: this.tokens,
        })
      );
    },

    hydrate() {
      if (import.meta.server) {
        return;
      }

      const persisted = readFromStorage();
      if (!persisted) {
        return;
      }

      const tokens =
        persisted.tokens && isIsoFuture(persisted.tokens.accessExpiresAt)
          ? persisted.tokens
          : null;

      this.identifier = persisted.identifier;
      this.deviceId = persisted.deviceId;
      this.deviceName = persisted.deviceName;
      this.tokens = tokens;
    },

    async login(params: LoginParameters) {
      if (this.loading) {
        return;
      }

      this.loading = true;
      this.resetErrors();

      const nuxtApp = useNuxtApp();
      const api = nuxtApp.$api;

      const body: LoginRequestBody = {
        identifier: params.identifier.trim(),
        secret: params.secret,
        device: {
          device_id: params.deviceId.trim(),
        },
      };

      if (params.deviceName?.trim()) {
        body.device.device_name = params.deviceName.trim();
      }

      try {
        const response = await api<LoginResponse>('/sessions/login', {
          method: 'POST',
          body,
        });

        this.identifier = body.identifier;
        this.deviceId = body.device.device_id;
        this.deviceName = body.device.device_name ?? '';
        this.tokens = {
          accessToken: response.access_token,
          accessExpiresAt: response.access_expires_at,
          refreshToken: response.refresh_token,
          refreshExpiresAt: response.refresh_expires_at,
        };

        this.persist();
      } catch (error) {
        const { message, fieldErrors } = formatLoginError(error);
        this.fieldErrors = fieldErrors;
        this.error = message;
        throw new Error(message);
      } finally {
        this.loading = false;
      }
    },

    logout() {
      this.tokens = null;
      this.resetErrors();
      this.persist();
    },

    clearAll() {
      this.identifier = '';
      this.deviceId = '';
      this.deviceName = '';
      this.tokens = null;
      this.persist();
    },
  },
});
