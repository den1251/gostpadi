/* ЛБ3, задача 1: одномерный массив.
 * Ручной ввод или заполнение rand() по выбору пользователя,
 * затем сумма, минимум и максимум. */
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

#define N_MAX 100

int main(void) {
    int a[N_MAX];
    int n = 0;
    int i = 0;
    int mode = 0;
    int sum = 0;
    int min = 0;
    int max = 0;

    while (1) {
        printf("размер массива (1..%d): ", N_MAX);
        if (scanf("%d", &n) == 1 && n >= 1 && n <= N_MAX) {
            break;
        }
        printf("нужно 1..%d, повтори\n", N_MAX);
        while (getchar() != '\n') {
        }
    }
    while (1) {
        printf("1 - ввести вручную, 2 - случайные числа: ");
        if (scanf("%d", &mode) == 1 && (mode == 1 || mode == 2)) {
            break;
        }
        printf("нужно 1 или 2\n");
        while (getchar() != '\n') {
        }
    }
    if (mode == 1) {
        for (i = 0; i < n; i++) {
            printf("a[%d] = ", i);
            scanf("%d", &a[i]);
        }
    } else {
        srand(time(NULL));
        for (i = 0; i < n; i++) {
            a[i] = rand() % 100 - 50;
        }
    }
    printf("массив:\n");
    for (i = 0; i < n; i++) {
        printf("%d ", a[i]);
    }
    printf("\n");
    sum = 0;
    min = a[0];
    max = a[0];
    for (i = 0; i < n; i++) {
        sum = sum + a[i];
        if (a[i] < min) {
            min = a[i];
        }
        if (a[i] > max) {
            max = a[i];
        }
    }
    printf("сумма = %d, min = %d, max = %d\n", sum, min, max);
    return 0;
}
