const stringifyFallback = (value: unknown): string => {
  try {
    return JSON.stringify(value);
  } catch {
    return 'Unexpected error';
  }
};

export const extractErrorMessage = (err: unknown): string => {
  if (!err) {
    return '';
  }

  if (err instanceof Error) {
    return err.message;
  }

  if (typeof err === 'string') {
    return err;
  }

  if (typeof err === 'object') {
    const maybeFetchError = err as {
      message?: string;
      data?: { error?: string; message?: string };
    };

    if (maybeFetchError.data?.message) {
      return maybeFetchError.data.message;
    }

    if (typeof maybeFetchError.data?.error === 'string') {
      return maybeFetchError.data.error;
    }

    if (typeof maybeFetchError.message === 'string') {
      return maybeFetchError.message;
    }
  }

  return stringifyFallback(err);
};
