export interface GuildPermissionSnapshot {
  role: string
  platformRoles: string[]
  canSendMessages: boolean
  canCreateChannels: boolean
  canManageGuild: boolean
}

const normalizeRole = (role?: string | null): string => {
  if (!role) {
    return ''
  }
  return role.trim().toLowerCase()
}

const capabilityMatrix: Record<string, Omit<GuildPermissionSnapshot, 'role' | 'platformRoles'>> = {
  owner: {
    canSendMessages: true,
    canCreateChannels: true,
    canManageGuild: true,
  },
  admin: {
    canSendMessages: true,
    canCreateChannels: true,
    canManageGuild: true,
  },
  moderator: {
    canSendMessages: true,
    canCreateChannels: true,
    canManageGuild: false,
  },
  maintainer: {
    canSendMessages: true,
    canCreateChannels: true,
    canManageGuild: false,
  },
  member: {
    canSendMessages: true,
    canCreateChannels: false,
    canManageGuild: false,
  },
  contributor: {
    canSendMessages: true,
    canCreateChannels: false,
    canManageGuild: false,
  },
  viewer: {
    canSendMessages: false,
    canCreateChannels: false,
    canManageGuild: false,
  },
  guest: {
    canSendMessages: false,
    canCreateChannels: false,
    canManageGuild: false,
  },
}

const platformAdminMatchers = [/admin/, /owner/, /superuser/, /maintainer/] as const

export const deriveGuildPermissions = (
  guildRole?: string | null,
  platformRoles: string[] = [],
): GuildPermissionSnapshot => {
  const normalizedRole = normalizeRole(guildRole)
  const normalizedPlatformRoles = platformRoles.map((role) => normalizeRole(role)).filter(Boolean)

  const isPlatformAdmin = normalizedPlatformRoles.some((role) =>
    platformAdminMatchers.some((matcher) => matcher.test(role)),
  )

  const capabilities = capabilityMatrix[normalizedRole] ?? capabilityMatrix.member

  return {
    role: normalizedRole,
    platformRoles: normalizedPlatformRoles,
    canSendMessages: capabilities.canSendMessages || isPlatformAdmin,
    canCreateChannels: capabilities.canCreateChannels || isPlatformAdmin,
    canManageGuild: capabilities.canManageGuild || isPlatformAdmin,
  }
}

export const permissionGuidance = (
  action: 'sendMessages' | 'createChannels' | 'adminPanel',
  snapshot: GuildPermissionSnapshot,
): string => {
  const friendlyRole = snapshot.role ? snapshot.role : 'member'

  if (action === 'sendMessages') {
    return `You need messaging rights in this guild. Ask an admin to upgrade your role (current role: ${friendlyRole}).`
  }

  if (action === 'createChannels') {
    return `Only guild moderators or admins can create channels. Your current role (${friendlyRole}) lacks that permission.`
  }

  return `Admin controls are hidden unless you are a guild admin or platform maintainer. Current role: ${friendlyRole}.`
}

export const resolveGuildRole = (
  guildId: string | null,
  guilds: Array<{ guildId: string; role?: string | null }> | undefined,
): string | null => {
  if (!guildId || !guilds?.length) {
    return null
  }

  const match = guilds.find((guild) => guild.guildId === guildId)
  return match?.role ?? null
}
