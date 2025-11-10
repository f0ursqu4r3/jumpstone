export interface GuildPermissionSnapshot {
  role: string
  guildRole: string | null
  channelRole: string | null
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
  channelRole?: string | null,
): GuildPermissionSnapshot => {
  const normalizedGuildRole = normalizeRole(guildRole)
  const normalizedChannelRole = normalizeRole(channelRole)
  const normalizedPlatformRoles = platformRoles.map((role) => normalizeRole(role)).filter(Boolean)

  const bestServerRole = normalizedPlatformRoles.length
    ? normalizedPlatformRoles.reduce((best, candidate) =>
        roleRank(candidate) > roleRank(best) ? candidate : best,
        normalizedPlatformRoles[0],
      )
    : null

  const selectedGuildRole = normalizedGuildRole || null
  const selectedChannelRole = normalizedChannelRole || null
  const effectiveRole =
    bestServerRole ?? selectedGuildRole ?? selectedChannelRole ?? 'member'

  const isPlatformAdmin = normalizedPlatformRoles.some((role) =>
    platformAdminMatchers.some((matcher) => matcher.test(role)),
  )

  const capabilities = capabilityMatrix[effectiveRole] ?? capabilityMatrix.member

  return {
    role: effectiveRole,
    guildRole: selectedGuildRole,
    channelRole: selectedChannelRole,
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

export const resolveChannelRole = (
  channelId: string | null,
  channels:
    | Array<{ channelId: string; role?: string | null; effectiveRole?: string | null }>
    | undefined,
): { role: string | null; effectiveRole: string | null } | null => {
  if (!channelId || !channels?.length) {
    return null
  }

  const match = channels.find((channel) => channel.channelId === channelId)
  if (!match) {
    return null
  }

  const normalizedRole = normalizeRole(match.role)
  const normalizedEffective = normalizeRole(match.effectiveRole ?? match.role)

  return {
    role: normalizedRole || null,
    effectiveRole: normalizedEffective || normalizedRole || null,
  }
}
