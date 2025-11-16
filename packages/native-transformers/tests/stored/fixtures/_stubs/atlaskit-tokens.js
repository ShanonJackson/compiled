module.exports = {
  token(path, fallback) {
    const map = {
      'color.text': '#172B4D',
      'color.border': '#E4E6EA',
      'spacing.scale.300': '24px',
    };
    return map[path] || fallback || path;
  },
};
