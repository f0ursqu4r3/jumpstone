import { computed, reactive, toRefs } from 'vue'
import { defineStore } from 'pinia'

import { extractErrorMessage } from '@/utils/errors'
import { getRuntimeConfig } from '@/config/runtime'
import type {
  ApiErrorResponse,
  CurrentUser,
  LoginParameters,
  LoginRequestBody,
  LoginResponse,
  RefreshRequestBody,
  RegisterParameters,
  RegisterRequestBody,
  RegisterResponse,
} from '~/types/session'

const STORAGE_KEY = 'openguild.session.v1'
const ACCESS_REFRESH_THRESHOLD_MS = 60_000
const PROFILE_ENDPOINTS = ['/client/v1/users/me', '/users/me'] as const

let resolvedProfileEndpoint: string | null = null
let refreshPromise: Promise<boolean> | null = null

interface SessionTokens {
  accessToken: string
  accessExpiresAt: string
  refreshToken: string
  refreshExpiresAt: string
}

interface StoredDevice {
  deviceId: string
  deviceName?: string | null
  lastSeenAt?: string | null
  ipAddress?: string | null
  userAgent?: string | null
}

interface StoredGuild {
  guildId: string
  name?: string | null
  role?: string | null
}

interface StoredProfile {
  userId: string
  username: string
  displayName: string
  avatarUrl?: string | null
  email?: string | null
  serverName?: string | null
  defaultGuildId?: string | null
  timezone?: string | null
  locale?: string | null
  createdAt?: string | null
  updatedAt?: string | null
  roles?: string[]
  guilds?: StoredGuild[]
  devices?: StoredDevice[]
  metadata?: Record<string, unknown>
}

interface PersistedSession {
  identifier: string
  deviceId: string
  deviceName: string
  tokens: SessionTokens | null
  profile: StoredProfile | null
}

interface SessionState extends PersistedSession {
  loading: boolean
  error: string | null
  fieldErrors: Record<string, string>
  hydrated: boolean
  profileLoading: boolean
  profileError: string | null
  profileFetchedAt: number | null
  refreshing: boolean
  refreshError: string | null
}

const isIsoFuture = (iso: string | null | undefined): boolean => {
  if (!iso) {
    return false
  }
  const parsed = Date.parse(iso)
  if (Number.isNaN(parsed)) {
    return false
  }
  return parsed > Date.now()
}

const msUntil = (iso: string | null | undefined): number => {
  if (!iso) {
    return Number.NEGATIVE_INFINITY
  }

  const parsed = Date.parse(iso)
  if (Number.isNaN(parsed)) {
    return Number.NEGATIVE_INFINITY
  }

  return parsed - Date.now()
}

const sanitizeTokens = (value: unknown): SessionTokens | null => {
  if (!value || typeof value !== 'object') {
    return null
  }

  const tokens = value as Partial<Record<keyof SessionTokens, unknown>>
  const accessToken =
    typeof tokens.accessToken === 'string' ? tokens.accessToken : ''
  const refreshToken =
    typeof tokens.refreshToken === 'string' ? tokens.refreshToken : ''
  const accessExpiresAt =
    typeof tokens.accessExpiresAt === 'string' ? tokens.accessExpiresAt : ''
  const refreshExpiresAt =
    typeof tokens.refreshExpiresAt === 'string' ? tokens.refreshExpiresAt : ''

  if (!accessToken || !refreshToken) {
    return null
  }

  return {
    accessToken,
    refreshToken,
    accessExpiresAt,
    refreshExpiresAt,
  }
}

const sanitizeDevices = (value: unknown): StoredDevice[] | undefined => {
  if (!Array.isArray(value)) {
    return undefined
  }

  const devices = value
    .map((entry) => {
      if (!entry || typeof entry !== 'object') {
        return null
      }

      const base = entry as Partial<StoredDevice> & {
        device_id?: string
        device_name?: string | null
        last_seen_at?: string | null
        ip_address?: string | null
        user_agent?: string | null
      }

      const deviceId =
        typeof base.deviceId === 'string'
          ? base.deviceId
          : typeof base.device_id === 'string'
          ? base.device_id
          : null

      if (!deviceId) {
        return null
      }

      return {
        deviceId,
        deviceName:
          typeof base.deviceName === 'string'
            ? base.deviceName
            : typeof base.device_name === 'string'
            ? base.device_name
            : null,
        lastSeenAt:
          typeof base.lastSeenAt === 'string'
            ? base.lastSeenAt
            : typeof base.last_seen_at === 'string'
            ? base.last_seen_at
            : null,
        ipAddress:
          typeof base.ipAddress === 'string'
            ? base.ipAddress
            : typeof base.ip_address === 'string'
            ? base.ip_address
            : null,
        userAgent:
          typeof base.userAgent === 'string'
            ? base.userAgent
            : typeof base.user_agent === 'string'
            ? base.user_agent
            : null,
      }
    })
    .filter(Boolean) as StoredDevice[]

  return devices.length ? devices : undefined
}

const sanitizeGuilds = (value: unknown): StoredGuild[] | undefined => {
  if (!Array.isArray(value)) {
    return undefined
  }

  const guilds = value
    .map((entry) => {
      if (!entry || typeof entry !== 'object') {
        return null
      }

      const base = entry as Partial<StoredGuild> & {
        guild_id?: string
        display_name?: string | null
      }

      const guildId =
        typeof base.guildId === 'string'
          ? base.guildId
          : typeof base.guild_id === 'string'
          ? base.guild_id
          : null

      if (!guildId) {
        return null
      }

      return {
        guildId,
        name:
          typeof base.name === 'string'
            ? base.name
            : typeof base.display_name === 'string'
            ? base.display_name
            : null,
        role: typeof base.role === 'string' ? base.role : null,
      }
    })
    .filter(Boolean) as StoredGuild[]

  return guilds.length ? guilds : undefined
}

const sanitizeProfile = (value: unknown): StoredProfile | null => {
  if (!value || typeof value !== 'object') {
    return null
  }

  const raw = value as Partial<StoredProfile>

  if (typeof raw.username !== 'string' || !raw.username) {
    return null
  }

  const displayName =
    typeof raw.displayName === 'string' && raw.displayName.trim().length
      ? raw.displayName.trim()
      : raw.username

  const profile: StoredProfile = {
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
    serverName:
      typeof raw.serverName === 'string'
        ? raw.serverName
        : raw.serverName ?? null,
    defaultGuildId:
      typeof raw.defaultGuildId === 'string'
        ? raw.defaultGuildId
        : raw.defaultGuildId ?? null,
    timezone:
      typeof raw.timezone === 'string' ? raw.timezone : raw.timezone ?? null,
    locale: typeof raw.locale === 'string' ? raw.locale : raw.locale ?? null,
    createdAt:
      typeof raw.createdAt === 'string' ? raw.createdAt : raw.createdAt ?? null,
    updatedAt:
      typeof raw.updatedAt === 'string' ? raw.updatedAt : raw.updatedAt ?? null,
    roles: Array.isArray(raw.roles)
      ? raw.roles.filter(
          (role): role is string => typeof role === 'string' && role.length > 0,
        )
      : undefined,
    guilds: sanitizeGuilds(raw.guilds),
    devices: sanitizeDevices(raw.devices),
    metadata:
      raw.metadata && typeof raw.metadata === 'object'
        ? raw.metadata
        : undefined,
  }

  return profile
}

const readFromStorage = (): PersistedSession | null => {
  if (typeof window === 'undefined') {
    return null
  }

  try {
    const raw = window.localStorage.getItem(STORAGE_KEY)
    if (!raw) {
      return null
    }

    const parsed = JSON.parse(raw) as Partial<PersistedSession>
    return {
      identifier:
        typeof parsed.identifier === 'string' ? parsed.identifier : '',
      deviceId: typeof parsed.deviceId === 'string' ? parsed.deviceId : '',
      deviceName:
        typeof parsed.deviceName === 'string' ? parsed.deviceName : '',
      tokens: sanitizeTokens(parsed.tokens),
      profile: sanitizeProfile(parsed.profile),
    }
  } catch {
    return null
  }
}

const writeToStorage = (value: PersistedSession) => {
  if (typeof window === 'undefined') {
    return
  }

  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(value))
  } catch {
    // ignore storage failures to avoid breaking login flows when storage is unavailable
  }
}

const baseState = (): SessionState => ({
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
  refreshing: false,
  refreshError: null,
})

const initialState = (): SessionState => {
  const state = baseState()

  const persisted = readFromStorage()
  if (!persisted) {
    return {
      ...state,
      hydrated: true,
    }
  }

  const tokens =
    persisted.tokens && isIsoFuture(persisted.tokens.accessExpiresAt)
      ? persisted.tokens
      : null

  return {
    ...state,
    identifier: persisted.identifier ?? '',
    deviceId: persisted.deviceId ?? '',
    deviceName: persisted.deviceName ?? '',
    tokens,
    profile: persisted.profile ?? null,
    hydrated: true,
    profileFetchedAt: persisted.profile ? Date.now() : null,
  }
}

const mapValidationField = (field: string): string => {
  if (field === 'device.device_id') {
    return 'deviceId'
  }
  if (field === 'device.device_name') {
    return 'deviceName'
  }
  return field
}

const formatLoginError = (
  err: unknown,
): { message: string; fieldErrors: Record<string, string> } => {
  const fieldErrors: Record<string, string> = {}
  const fallbackMessage =
    'Unable to sign in right now. Please try again in a moment.'

  const maybeFetchError = err as {
    data?: ApiErrorResponse
    response?: { status?: number }
  }

  const { data } = maybeFetchError

  if (data?.details) {
    data.details.forEach((detail) => {
      if (!detail || typeof detail.field !== 'string') {
        return
      }
      const key = mapValidationField(detail.field)
      fieldErrors[key] = detail.message ?? 'Invalid value'
    })
  }

  if (data?.error === 'invalid_credentials') {
    return {
      message:
        'Invalid credentials. Check your identifier and secret, then try again.',
      fieldErrors,
    }
  }

  if (data?.error === 'validation_error') {
    return {
      message:
        Object.values(fieldErrors)[0] ??
        'Please fix the highlighted fields and try again.',
      fieldErrors,
    }
  }

  if (data?.message) {
    return {
      message: data.message,
      fieldErrors,
    }
  }

  if (maybeFetchError.response?.status === 401) {
    return {
      message:
        'Invalid credentials. Check your identifier and secret, then try again.',
      fieldErrors,
    }
  }

  return {
    message: extractErrorMessage(err) || fallbackMessage,
    fieldErrors,
  }
}

const formatRegisterError = (
  err: unknown,
): { message: string; fieldErrors: Record<string, string> } => {
  const fieldErrors: Record<string, string> = {}
  const fallbackMessage =
    'Unable to create an account right now. Please try again shortly.'

  const maybeFetchError = err as {
    data?: ApiErrorResponse
    response?: { status?: number }
  }

  const { data } = maybeFetchError

  if (data?.details) {
    data.details.forEach((detail) => {
      if (!detail || typeof detail.field !== 'string') {
        return
      }
      fieldErrors[detail.field] = detail.message ?? 'Invalid value'
    })
  }

  if (data?.error === 'username_taken') {
    if (!fieldErrors.username) {
      fieldErrors.username = 'This username is already in use.'
    }
    return {
      message: 'That username is already in use. Choose a different one.',
      fieldErrors,
    }
  }

  if (data?.error === 'database_unavailable') {
    return {
      message:
        'Registration is temporarily unavailable while the database is offline.',
      fieldErrors,
    }
  }

  if (data?.error === 'server_error') {
    return {
      message:
        'Registration failed due to a server error. Please try again soon.',
      fieldErrors,
    }
  }

  if (data?.error === 'validation_error') {
    return {
      message:
        Object.values(fieldErrors)[0] ??
        'Please fix the highlighted fields and try again.',
      fieldErrors,
    }
  }

  if (data?.message) {
    return {
      message: data.message,
      fieldErrors,
    }
  }

  return {
    message: extractErrorMessage(err) || fallbackMessage,
    fieldErrors,
  }
}

const toPersistedSession = (state: PersistedSession): PersistedSession => ({
  identifier: state.identifier,
  deviceId: state.deviceId,
  deviceName: state.deviceName,
  tokens: state.tokens,
  profile: state.profile,
})

const unwrapCurrentUser = (
  payload: CurrentUser | { user: CurrentUser },
): CurrentUser => {
  if (
    payload &&
    typeof payload === 'object' &&
    'user' in payload &&
    payload.user
  ) {
    return payload.user
  }

  return payload as CurrentUser
}

const mapCurrentUser = (payload: CurrentUser): StoredProfile => ({
  userId: payload.user_id,
  username: payload.username,
  displayName:
    payload.display_name && payload.display_name.trim().length
      ? payload.display_name.trim()
      : payload.username,
  avatarUrl: payload.avatar_url ?? null,
  email: payload.email ?? null,
  serverName: payload.server_name ?? null,
  defaultGuildId: payload.default_guild_id ?? null,
  timezone: payload.timezone ?? null,
  locale: payload.locale ?? null,
  createdAt: payload.created_at ?? null,
  updatedAt: payload.updated_at ?? null,
  roles: Array.isArray(payload.roles)
    ? payload.roles.filter(
        (role): role is string => typeof role === 'string' && role.length > 0,
      )
    : undefined,
  guilds: sanitizeGuilds(payload.guilds),
  devices: sanitizeDevices(payload.devices),
  metadata:
    payload.metadata && typeof payload.metadata === 'object'
      ? payload.metadata
      : undefined,
})

const createRequestId = () => {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID()
  }

  return Math.random().toString(16).slice(2)
}

const parseResponse = async (response: Response) => {
  const contentType = response.headers.get('content-type') ?? ''
  if (contentType.includes('application/json')) {
    return response.json().catch(() => null)
  }

  return response.text().catch(() => null)
}

const createRequestError = async (response: Response) => {
  const data = await parseResponse(response)
  const messageFromData =
    data &&
    typeof data === 'object' &&
    'message' in data &&
    typeof (data as Record<string, unknown>).message === 'string'
      ? (data as { message: string }).message
      : null

  const error = new Error(
    messageFromData ?? `Request failed with status ${response.status}`,
  ) as Error & {
    data?: unknown
    response?: { status?: number }
  }

  error.data = data ?? null
  error.response = { status: response.status }
  return error
}

export const useSessionStore = defineStore('session', () => {
  const state = reactive<SessionState>(initialState())
  const runtimeConfig = getRuntimeConfig()
  const apiBaseUrl = runtimeConfig.public.apiBaseUrl.replace(/\/$/, '')

  const buildUrl = (path: string) => {
    if (path.startsWith('http')) {
      return path
    }
    if (!path.startsWith('/')) {
      return `${apiBaseUrl}/${path}`
    }
    return `${apiBaseUrl}${path}`
  }

  const isAuthenticated = computed(() => {
    if (!state.tokens) {
      return false
    }
    if (!state.tokens.accessToken) {
      return false
    }
    return isIsoFuture(state.tokens.accessExpiresAt)
  })

  const accessToken = computed(() => state.tokens?.accessToken ?? '')
  const displayName = computed(
    () => state.profile?.displayName ?? state.identifier,
  )
  const profileAvatar = computed(() => state.profile?.avatarUrl ?? null)

  function resetErrors() {
    state.error = null
    state.fieldErrors = {}
  }

  function persist() {
    writeToStorage(
      toPersistedSession({
        identifier: state.identifier,
        deviceId: state.deviceId,
        deviceName: state.deviceName,
        tokens: state.tokens,
        profile: state.profile,
      }),
    )
  }

  function hydrate() {
    const persisted = readFromStorage()
    if (!persisted) {
      state.hydrated = true
      return
    }

    const tokens =
      persisted.tokens && isIsoFuture(persisted.tokens.accessExpiresAt)
        ? persisted.tokens
        : null

    state.identifier = persisted.identifier
    state.deviceId = persisted.deviceId
    state.deviceName = persisted.deviceName
    state.tokens = tokens
    state.profile = persisted.profile
    state.profileError = null
    state.profileLoading = false
    state.profileFetchedAt = persisted.profile ? Date.now() : null
    state.refreshing = false
    state.refreshError = null
    state.hydrated = true
  }

  const request = async <T>(
    path: string,
    init: RequestInit = {},
    options: { skipAuth?: boolean } = {},
  ): Promise<T> => {
    const headers = new Headers(init.headers)

    if (!headers.has('accept')) {
      headers.set('accept', 'application/json')
    }

    if (!options.skipAuth && state.tokens?.accessToken) {
      headers.set('authorization', `Bearer ${state.tokens.accessToken}`)
    }

    if (state.deviceId && !headers.has('x-device-id')) {
      headers.set('x-device-id', state.deviceId)
    }

    if (!headers.has('x-request-id')) {
      headers.set('x-request-id', createRequestId())
    }

    const response = await fetch(buildUrl(path), {
      ...init,
      headers,
    })

    if (!response.ok) {
      throw await createRequestError(response)
    }

    return (await parseResponse(response)) as T
  }

  async function login(params: LoginParameters) {
    if (state.loading) {
      return
    }

    state.loading = true
    resetErrors()
    state.refreshError = null

    const body: LoginRequestBody = {
      identifier: params.identifier.trim(),
      secret: params.secret,
      device: {
        device_id: params.deviceId.trim(),
      },
    }

    if (params.deviceName?.trim()) {
      body.device.device_name = params.deviceName.trim()
    }

    try {
      const response = await request<LoginResponse>(
        '/sessions/login',
        {
          method: 'POST',
          body: JSON.stringify(body),
          headers: {
            'content-type': 'application/json',
          },
        },
        { skipAuth: true },
      )

      state.identifier = body.identifier
      state.deviceId = body.device.device_id
      state.deviceName = body.device.device_name ?? ''
      state.tokens = {
        accessToken: response.access_token,
        accessExpiresAt: response.access_expires_at,
        refreshToken: response.refresh_token,
        refreshExpiresAt: response.refresh_expires_at,
      }
      state.hydrated = true

      await fetchProfile(true).catch((err) => {
        console.error('Failed to load profile after login', err)
      })

      persist()
    } catch (error) {
      const { message, fieldErrors } = formatLoginError(error)
      state.fieldErrors = fieldErrors
      state.error = message
      throw new Error(message)
    } finally {
      state.loading = false
    }
  }

  async function register(params: RegisterParameters) {
    if (state.loading) {
      return
    }

    state.loading = true
    resetErrors()
    state.refreshError = null

    const username = params.username.trim()
    const deviceId = params.deviceId.trim()
    const deviceName = params.deviceName?.trim()
    const body: RegisterRequestBody = {
      username,
      password: params.password,
    }

    try {
      await request<RegisterResponse>(
        '/users/register',
        {
          method: 'POST',
          body: JSON.stringify(body),
          headers: {
            'content-type': 'application/json',
          },
        },
        { skipAuth: true },
      )
    } catch (error) {
      const { message, fieldErrors } = formatRegisterError(error)
      state.fieldErrors = fieldErrors
      state.error = message
      throw new Error(message)
    } finally {
      state.loading = false
    }

    return login({
      identifier: username,
      secret: body.password,
      deviceId,
      deviceName,
    })
  }

  function logout() {
    state.tokens = null
    resetErrors()
    state.hydrated = true
    state.profile = null
    state.profileError = null
    state.profileLoading = false
    state.profileFetchedAt = null
    state.refreshing = false
    state.refreshError = null
    persist()
  }

  function clearAll() {
    state.identifier = ''
    state.deviceId = ''
    state.deviceName = ''
    state.tokens = null
    state.profile = null
    state.profileError = null
    state.profileLoading = false
    state.profileFetchedAt = null
    state.refreshing = false
    state.refreshError = null
    state.hydrated = true
    persist()
  }

  function needsAccessRefresh(
    thresholdMs = ACCESS_REFRESH_THRESHOLD_MS,
  ): boolean {
    if (!state.tokens?.accessExpiresAt) {
      return false
    }

    const delta = msUntil(state.tokens.accessExpiresAt)
    return delta <= thresholdMs
  }

  async function ensureFreshAccessToken(): Promise<boolean> {
    if (!isAuthenticated.value) {
      return false
    }

    if (!needsAccessRefresh()) {
      return true
    }

    return refreshTokens()
  }

  async function refreshTokens(force = false): Promise<boolean> {
    if (!state.tokens) {
      return false
    }

    if (!state.tokens.refreshToken) {
      return false
    }

    if (!force && !needsAccessRefresh()) {
      return true
    }

    if (!isIsoFuture(state.tokens.refreshExpiresAt)) {
      logout()
      return false
    }

    if (refreshPromise) {
      return refreshPromise
    }

    refreshPromise = (async () => {
      state.refreshing = true
      state.refreshError = null

      try {
        const body: RefreshRequestBody = {
          refresh_token: state.tokens!.refreshToken,
        }

        const response = await request<LoginResponse>(
          '/sessions/refresh',
          {
            method: 'POST',
            body: JSON.stringify(body),
            headers: {
              'content-type': 'application/json',
            },
          },
          { skipAuth: true },
        )

        state.tokens = {
          accessToken: response.access_token,
          accessExpiresAt: response.access_expires_at,
          refreshToken: response.refresh_token,
          refreshExpiresAt: response.refresh_expires_at,
        }

        persist()
        return true
      } catch (error) {
        state.refreshError = extractErrorMessage(error)
        const maybeFetchError = error as { response?: { status?: number } }
        if (maybeFetchError.response?.status === 401) {
          logout()
        }
        return false
      } finally {
        state.refreshing = false
        refreshPromise = null
      }
    })()

    return refreshPromise
  }

  async function fetchProfile(force = false): Promise<StoredProfile | null> {
    if (!isAuthenticated.value) {
      state.profile = null
      state.profileFetchedAt = null
      persist()
      return null
    }

    if (state.profileLoading) {
      return state.profile
    }

    if (
      !force &&
      state.profile &&
      state.profileFetchedAt &&
      Date.now() - state.profileFetchedAt < 60_000
    ) {
      return state.profile
    }

    state.profileLoading = true
    state.profileError = null

    try {
      const endpoints = resolvedProfileEndpoint
        ? [resolvedProfileEndpoint]
        : PROFILE_ENDPOINTS

      let payload: CurrentUser | { user: CurrentUser } | null = null
      let lastError: unknown = null

      for (const endpoint of endpoints) {
        try {
          const data = await request<CurrentUser | { user: CurrentUser }>(
            endpoint,
          )
          resolvedProfileEndpoint = endpoint
          payload = data
          break
        } catch (err) {
          lastError = err
          const status = (err as { response?: { status?: number } })?.response
            ?.status
          if (status === 404) {
            continue
          }
          throw err
        }
      }

      if (!payload) {
        throw lastError ?? new Error('Profile endpoint unavailable')
      }

      const currentUser = unwrapCurrentUser(payload)
      const profile = mapCurrentUser(currentUser)
      state.profile = profile
      state.profileFetchedAt = Date.now()
      persist()
      return profile
    } catch (error) {
      state.profileError = extractErrorMessage(error)
      const maybeFetchError = error as { response?: { status?: number } }
      if (maybeFetchError.response?.status === 401) {
        logout()
      }
      throw error
    } finally {
      state.profileLoading = false
    }
  }

  const refs = toRefs(state)

  return {
    ...refs,
    isAuthenticated,
    accessToken,
    displayName,
    profileAvatar,
    resetErrors,
    persist,
    hydrate,
    login,
    register,
    logout,
    clearAll,
    needsAccessRefresh,
    ensureFreshAccessToken,
    refreshTokens,
    fetchProfile,
  }
})
