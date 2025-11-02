import { defineStore } from 'pinia';
import { extractErrorMessage } from '~/utils/errors';
import type {
  ApiErrorResponse,
  CurrentUser,
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

interface StoredProfile {
  userId: string;
  username: string;
  displayName: string;
  avatarUrl?: string | null;
  email?: string | null;
}

interface PersistedSession {
  identifier: string;
  deviceId: string;
  deviceName: string;
  tokens: SessionTokens | null;
  profile: StoredProfile | null;
}

interface SessionState extends PersistedSession {
  loading: boolean;
  error: string | null;
  fieldErrors: Record<string, string>;
  hydrated: boolean;
  profileLoading: boolean;
  profileError: string | null;
  profileFetchedAt: number | null;
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

const sanitizeProfile = (value: unknown): StoredProfile | null => {
  if (!value || typeof value !== 'object') {
    return null;
  }

  const raw = value as Partial<StoredProfile>;

  if (typeof raw.username !== 'string' || !raw.username) {
    return null;
  }

  const displayName =
    typeof raw.displayName === 'string' && raw.displayName.trim().length
      ? raw.displayName.trim()
      : raw.username;

  return {
    userId:
      typeof raw.userId === 'string' && raw.userId.length
        ? raw.userId
        : raw.username,
    username: raw.username,
    displayName,
    avatarUrl:
      typeof raw.avatarUrl === 'string'
        ? raw.avatarUrl || null
        : raw.avatarUrl ?? null,
    email: typeof raw.email === 'string' ? raw.email : null,
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
      profile: sanitizeProfile(parsed.profile),
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
      profile: null,
      loading: false,
      error: null,
      fieldErrors: {},
      hydrated: false,
      profileLoading: false,
      profileError: null,
      profileFetchedAt: null,
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
    profile: persisted?.profile ?? null,
    loading: false,
    error: null,
    fieldErrors: {},
    hydrated: true,
    profileLoading: false,
    profileError: null,
    profileFetchedAt: persisted?.profile ? Date.now() : null,
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
  profile: state.profile,
});

const mapCurrentUser = (payload: CurrentUser): StoredProfile => ({
  userId: payload.user_id,
  username: payload.username,
  displayName:
    payload.display_name && payload.display_name.trim().length
      ? payload.display_name.trim()
      : payload.username,
  avatarUrl: payload.avatar_url ?? null,
  email: payload.email ?? null,
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
    displayName: (state): string => state.profile?.displayName ?? state.identifier,
    profileAvatar: (state): string | null => state.profile?.avatarUrl ?? null,
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
          profile: this.profile,
        })
      );
    },

    hydrate() {
      if (import.meta.server) {
        return;
      }

      const persisted = readFromStorage();
      if (!persisted) {
        this.hydrated = true;
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
      this.profile = persisted.profile;
      this.profileError = null;
      this.profileLoading = false;
      this.profileFetchedAt = persisted.profile ? Date.now() : null;
      this.hydrated = true;
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
        this.hydrated = true;

        await this.fetchProfile(true).catch((err) => {
          console.error('Failed to load profile after login', err);
        });

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
      this.hydrated = true;
      this.profile = null;
      this.profileError = null;
      this.profileLoading = false;
      this.profileFetchedAt = null;
      this.persist();
    },

    clearAll() {
      this.identifier = '';
      this.deviceId = '';
      this.deviceName = '';
      this.tokens = null;
      this.profile = null;
      this.profileError = null;
      this.profileLoading = false;
      this.profileFetchedAt = null;
      this.hydrated = true;
      this.persist();
    },

    async fetchProfile(force = false): Promise<StoredProfile | null> {
      if (!this.isAuthenticated) {
        this.profile = null;
        this.profileFetchedAt = null;
        this.persist();
        return null;
      }

      if (this.profileLoading) {
        return this.profile;
      }

      if (
        !force &&
        this.profile &&
        this.profileFetchedAt &&
        Date.now() - this.profileFetchedAt < 60_000
      ) {
        return this.profile;
      }

      const nuxtApp = useNuxtApp();
      const api = nuxtApp.$api;

      this.profileLoading = true;
      this.profileError = null;

      try {
        const payload = await api<CurrentUser>('/users/me');
        const profile = mapCurrentUser(payload);
        this.profile = profile;
        this.profileFetchedAt = Date.now();
        this.persist();
        return profile;
      } catch (error) {
        this.profileError = extractErrorMessage(error);
        const maybeFetchError = error as { response?: { status?: number } };
        if (maybeFetchError.response?.status === 401) {
          this.logout();
        }
        throw error;
      } finally {
        this.profileLoading = false;
      }
    },
  },
});
