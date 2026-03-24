/** Map ExHentai category names to colors matching the site's scheme. */
const CATEGORY_COLORS: Record<string, string> = {
  "Doujinshi": "#f44336",
  "Manga": "#ff9800",
  "Artist CG": "#ffc107",
  "Game CG": "#4caf50",
  "Western": "#8bc34a",
  "Non-H": "#2196f3",
  "Image Set": "#3f51b5",
  "Cosplay": "#9c27b0",
  "Asian Porn": "#e91e63",
  "Misc": "#9e9e9e",
  "Private": "#000000",
};

export function categoryColor(category: string): string {
  return CATEGORY_COLORS[category] ?? "#9e9e9e";
}
