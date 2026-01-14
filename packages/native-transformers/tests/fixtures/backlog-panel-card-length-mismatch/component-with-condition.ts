export const componentWithCondition = (condition: () => boolean, onTrue: any, onFalse: any) =>
  condition() ? onTrue : onFalse;
