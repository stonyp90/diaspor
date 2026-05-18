export {
  loadTagSuggestionConfig,
  saveTagSuggestionConfig,
  isMountedStorage,
  isVideoFile,
  suggestTags,
  requestTagSuggestions,
  startBackgroundTagSuggestions,
  stopBackgroundTagSuggestions,
} from './tag-suggestion.service';

export type {
  TagSuggestionConfig,
  SuggestedTag,
  TagSuggestionRequest,
  TagSuggestionResult,
} from './tag-suggestion.service';
